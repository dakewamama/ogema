use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;

use crate::pump_buy::{build_buy, BuyAccounts};
use crate::exit_monitor::{watch, ExitRules};
use crate::bundle;

const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

fn micro_rules() -> ExitRules {
    ExitRules { take_profit_mult: 2.0, stop_loss_mult: 0.6, trailing_pct: 0.25, max_hold_secs: 180, poll_ms: 400 }
}

fn bonding_curve_pda(mint: &Pubkey) -> Pubkey {
    let program = Pubkey::from_str(PUMP_PROGRAM).unwrap();
    Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &program).0
}

fn read_curve(rpc: &RpcClient, mint: &Pubkey) -> Result<(Pubkey, f64, bool)> {
    let bc = bonding_curve_pda(mint);
    let data = rpc.get_account_data(&bc)?;
    if data.len() < 81 { return Err(anyhow!("curve account too small")); }
    let v_token = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let v_quote = u64::from_le_bytes(data[16..24].try_into().unwrap());
    let complete = data[48] != 0;
    let creator = Pubkey::try_from(&data[49..81]).map_err(|_| anyhow!("bad creator"))?;
    if v_token == 0 { return Err(anyhow!("zero token reserves")); }
    Ok((creator, (v_quote as f64 / 1e9) / (v_token as f64 / 1e6), complete))
}

pub struct SnipeConfig {
    pub sol_in_lamports: u64,
    pub fee_recipient:   Pubkey,
    pub tip_lamports:    u64,
}

pub async fn execute(rpc: &RpcClient, kp: &Keypair, mint: Pubkey, cfg: &SnipeConfig) -> Result<f64> {
    let (creator, entry_price, complete) = read_curve(rpc, &mint)?;
    if complete { return Err(anyhow!("curve already migrated")); }

    let sol_in = cfg.sol_in_lamports as f64 / 1e9;
    let est_tokens = (sol_in / entry_price) as u64 * 1_000_000;
    let max_sol_cost = (cfg.sol_in_lamports as f64 * 1.15) as u64;

    let buy_ix = build_buy(
        &BuyAccounts { mint, user: kp.pubkey(), creator, fee_recipient: cfg.fee_recipient },
        est_tokens, max_sol_cost, true,
    );

    let landed = bundle::send(bundle::build(kp, rpc, buy_ix, cfg.tip_lamports)).await;
    let Some((bundle_id, ep)) = landed else { return Err(anyhow!("buy bundle did not land")) };
    eprintln!("snipe buy landed via {ep} | {bundle_id} | entry={:.9}", entry_price);

    let token_amount = est_tokens;

    let (sell_ix, reason, pnl_mult) = watch(
        rpc, kp, mint, creator, cfg.fee_recipient, entry_price, token_amount, micro_rules(),
    ).await?;

    match bundle::send(bundle::build(kp, rpc, sell_ix, cfg.tip_lamports)).await {
        Some((id, ep)) => eprintln!("snipe sell ({}) via {ep} | {id} | {:.2}x", reason.label(), pnl_mult),
        None           => eprintln!("WARN: sell bundle did not land, position may be stuck, retry manually"),
    }

    Ok(pnl_mult)
}
