use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::pump_sell::{build_sell, SellAccounts};

const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

pub struct ExitRules {
    pub take_profit_mult: f64,
    pub stop_loss_mult:   f64,
    pub trailing_pct:     f64,
    pub max_hold_secs:    u64,
    pub poll_ms:          u64,
}

impl Default for ExitRules {
    fn default() -> Self {
        Self { take_profit_mult: 2.5, stop_loss_mult: 0.6, trailing_pct: 0.30, max_hold_secs: 300, poll_ms: 500 }
    }
}

fn curve_price(rpc: &RpcClient, bonding_curve: &Pubkey) -> Result<f64> {
    let data = rpc.get_account_data(bonding_curve)?;
    if data.len() < 24 { return Err(anyhow!("bonding curve account too small")); }
    let v_token = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let v_sol   = u64::from_le_bytes(data[16..24].try_into().unwrap());
    if v_token == 0 { return Err(anyhow!("zero token reserves")); }
    Ok((v_sol as f64 / 1e9) / (v_token as f64 / 1e6))
}

fn bonding_curve_pda(mint: &Pubkey) -> Pubkey {
    let program = Pubkey::from_str(PUMP_PROGRAM).unwrap();
    Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &program).0
}

pub enum ExitReason { TakeProfit, StopLoss, Trailing, Timeout }

impl ExitReason {
    pub fn label(&self) -> &'static str {
        match self {
            ExitReason::TakeProfit => "take-profit",
            ExitReason::StopLoss   => "stop-loss",
            ExitReason::Trailing   => "trailing-stop",
            ExitReason::Timeout    => "timeout",
        }
    }
}

pub async fn watch(
    rpc: &RpcClient, kp: &Keypair, mint: Pubkey, creator: Pubkey, fee_recipient: Pubkey,
    entry_price: f64, token_amount: u64, rules: ExitRules,
) -> Result<(solana_sdk::instruction::Instruction, ExitReason, f64)> {
    let bc = bonding_curve_pda(&mint);
    let start = Instant::now();
    let mut peak = entry_price;

    loop {
        sleep(Duration::from_millis(rules.poll_ms)).await;
        let price = match curve_price(rpc, &bc) { Ok(p) => p, Err(_) => continue };
        if price > peak { peak = price; }

        let reason = if price >= entry_price * rules.take_profit_mult { Some(ExitReason::TakeProfit) }
            else if price <= entry_price * rules.stop_loss_mult { Some(ExitReason::StopLoss) }
            else if price <= peak * (1.0 - rules.trailing_pct) { Some(ExitReason::Trailing) }
            else if start.elapsed().as_secs() >= rules.max_hold_secs { Some(ExitReason::Timeout) }
            else { None };

        if let Some(reason) = reason {
            let expected_sol = price * (token_amount as f64 / 1e6);
            let min_sol_output = (expected_sol * 0.95 * 1e9) as u64;
            let ix = build_sell(&SellAccounts { mint, user: kp.pubkey(), creator, fee_recipient }, token_amount, min_sol_output);
            let pnl_mult = price / entry_price;
            eprintln!("exit: {} at {:.2}x (entry={:.9} now={:.9} peak={:.9})", reason.label(), pnl_mult, entry_price, price, peak);
            return Ok((ix, reason, pnl_mult));
        }
    }
}
