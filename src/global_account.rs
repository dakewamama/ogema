use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

// Global account layout verified against the live on-chain IDL (fetched
// from the IDL PDA) and cross-checked against the real Global state account:
// offset 8 (disc) + 1 (initialized: bool) + 32 (authority: pubkey) = 41,
// fee_recipient is the 32 bytes at offset 41. Confirmed the decoded pubkey
// (62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV) is a real System-Program
// wallet holding ~18,141 SOL, consistent with a live fee-collection account.
pub fn fetch_fee_recipient(rpc: &RpcClient) -> Result<Pubkey> {
    let program = Pubkey::from_str(PUMP_PROGRAM).unwrap();
    let (global, _) = Pubkey::find_program_address(&[b"global"], &program);
    let data = rpc.get_account_data(&global)?;

    let offset = 8 + 1 + 32;
    if data.len() < offset + 32 {
        return Err(anyhow!("global account too small, layout may have changed"));
    }
    Pubkey::try_from(&data[offset..offset + 32]).map_err(|_| anyhow!("bad fee_recipient bytes"))
}
