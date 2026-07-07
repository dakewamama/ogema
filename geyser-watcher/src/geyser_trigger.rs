use anyhow::Result;
use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterAccounts,
};
use std::collections::HashMap;

use crate::signal::ArbSignal;

const ENDPOINT: &str = "https://solana-mainnet-grpc.gateway.tatum.io:443";

const SOL_VAULT:  &str = "DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz";
const USDC_VAULT: &str = "HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz";

const SOL_DECIMALS:  f64 = 1e9;
const USDC_DECIMALS: f64 = 1e6;
const DEVIATION_THRESHOLD: f64 = 0.001;

fn spl_amount(data: &[u8]) -> Option<u64> {
    if data.len() < 72 { return None; }
    Some(u64::from_le_bytes(data[64..72].try_into().ok()?))
}

fn account_sub() -> SubscribeRequest {
    let mut accounts = HashMap::new();
    accounts.insert("raydium_vaults".to_string(), SubscribeRequestFilterAccounts {
        account: vec![SOL_VAULT.to_string(), USDC_VAULT.to_string()],
        owner: vec![], filters: vec![], nonempty_txn_signature: None, cuckoo_accounts_filter: None,
    });
    SubscribeRequest { accounts, commitment: Some(CommitmentLevel::Processed as i32), ..Default::default() }
}

pub async fn run(tx: Sender<ArbSignal>, tatum_key: String, pyth_price: impl Fn() -> f64 + Send) {
    loop {
        if let Err(e) = stream(&tx, &tatum_key, &pyth_price).await {
            eprintln!("geyser_trigger: {e:#} — reconnecting in 3s");
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }
}

async fn stream(tx: &Sender<ArbSignal>, tatum_key: &str, pyth_price: &(impl Fn() -> f64 + Send)) -> Result<()> {
    let mut client = GeyserGrpcClient::build_from_shared(ENDPOINT.to_string())?
        .tls_config(ClientTlsConfig::new().with_native_roots())?
        .x_token(Some(tatum_key.to_string()))?
        .connect().await?;

    eprintln!("geyser_trigger: watching Raydium SOL/USDC vaults");
    let (mut _sink, mut updates) = client.subscribe_with_request(Some(account_sub())).await?;

    let mut sol_reserve:  Option<u64> = None;
    let mut usdc_reserve: Option<u64> = None;

    while let Some(Ok(msg)) = updates.next().await {
        let UpdateOneof::Account(acc) = msg.update_oneof.unwrap_or(UpdateOneof::Ping(Default::default())) else { continue };
        let Some(info) = acc.account else { continue };
        let pubkey = bs58::encode(&info.pubkey).into_string();
        let Some(amount) = spl_amount(&info.data) else { continue };

        match pubkey.as_str() {
            SOL_VAULT  => sol_reserve  = Some(amount),
            USDC_VAULT => usdc_reserve = Some(amount),
            _ => continue,
        }

        let (Some(sol), Some(usdc)) = (sol_reserve, usdc_reserve) else { continue };
        if sol == 0 { continue; }

        let pool_price = (usdc as f64 / USDC_DECIMALS) / (sol as f64 / SOL_DECIMALS);
        let fair = pyth_price();
        if fair <= 0.0 { continue; }

        let deviation = (pool_price - fair) / fair;
        if deviation.abs() < DEVIATION_THRESHOLD { continue; }

        let direction: i8 = if pool_price < fair { 1 } else { -1 };
        let (reserve_in, reserve_out) = if direction > 0 { (usdc, sol) } else { (sol, usdc) };

        let signal = ArbSignal { pool_price, pyth_price: fair, deviation, direction, reserve_in, reserve_out, fair_price: fair };
        eprintln!("geyser_trigger: dev={:.4}% pool={:.2} pyth={:.2} dir={}", deviation*100.0, pool_price, fair, direction);

        if tx.send(signal).await.is_err() { return Ok(()); }
    }
    Ok(())
}
