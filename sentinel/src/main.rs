mod signal;
mod watchlist;

use anyhow::Result;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterTransactions,
};

const ENDPOINT:    &str = "https://solana-mainnet-grpc.gateway.tatum.io:443";
const SYSTEM_PROG: &str = "11111111111111111111111111111111";
const MIN_INBOUND: u64  = 50_000_000; // 0.05 SOL production threshold

fn subscription() -> SubscribeRequest {
    let mut txs = HashMap::new();
    txs.insert("inbound".to_string(), SubscribeRequestFilterTransactions {
        vote:             Some(false),
        failed:           Some(false),
        signature:        None,
        token_accounts:   None,
        account_include:  watchlist::WATCHLIST.iter().map(|s| s.to_string()).collect(),
        account_exclude:  vec![],
        account_required: vec![],
    });
    SubscribeRequest { transactions: txs, commitment: Some(CommitmentLevel::Processed as i32), ..Default::default() }
}

fn sol_transfer_lamports(data: &[u8]) -> Option<u64> {
    if data.len() < 12 { return None; }
    if u32::from_le_bytes(data[..4].try_into().ok()?) != 2 { return None; }
    Some(u64::from_le_bytes(data[4..12].try_into().ok()?))
}

fn inbound_hits(update: &yellowstone_grpc_proto::geyser::SubscribeUpdateTransaction) -> Vec<(String, u64)> {
    let Some(tx)  = update.transaction.as_ref() else { return vec![] };
    let Some(msg) = tx.transaction.as_ref().and_then(|t| t.message.as_ref()) else { return vec![] };
    let wl: HashSet<_> = watchlist::WATCHLIST.iter().copied().collect();
    let keys: Vec<String> = msg.account_keys.iter().map(|k| bs58::encode(k).into_string()).collect();

    msg.instructions.iter().filter_map(|ix| {
        let prog = keys.get(ix.program_id_index as usize)?.as_str();
        if prog != SYSTEM_PROG { return None; }
        let lamps = sol_transfer_lamports(&ix.data)?;
        if lamps < MIN_INBOUND { return None; }
        let dest = keys.get(*ix.accounts.get(1)? as usize)?.clone();
        wl.contains(dest.as_str()).then_some((dest, lamps))
    }).collect()
}

async fn stream(key: &str) -> Result<()> {
    let mut client = GeyserGrpcClient::build_from_shared(ENDPOINT.to_string())?
        .tls_config(ClientTlsConfig::new().with_native_roots())?
        .x_token(Some(key.to_string()))?
        .connect().await?;

    eprintln!("connected | watching {} wallets", watchlist::WATCHLIST.len());
    let (mut _sink, mut updates) = client.subscribe_with_request(Some(subscription())).await?;

    while let Some(Ok(msg)) = updates.next().await {
        if let Some(UpdateOneof::Transaction(tx)) = msg.update_oneof {
            let slot = tx.slot;
            for (pubkey, lamports) in inbound_hits(&tx) {
                println!("SIGNAL slot={slot} wallet={pubkey} inbound_sol={:.6}", lamports as f64 / 1e9);
                signal::fire(&pubkey, lamports, slot);
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider().install_default().expect("rustls provider");
    let key = std::env::var("TATUM_KEY").expect("TATUM_KEY not set");
    loop {
        if let Err(e) = stream(&key).await {
            eprintln!("stream error: {e:#} — reconnecting in 3s");
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }
}
