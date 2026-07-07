use solana_sdk::{instruction::{AccountMeta, Instruction}, pubkey::Pubkey};
use std::str::FromStr;
use base64::{Engine as _, engine::general_purpose};

const QUOTE: &str = "https://quote-api.jup.ag/v6/quote";
const SWAP:  &str = "https://quote-api.jup.ag/v6/swap-instructions";

pub async fn swap_ix(wallet: &str, inp: &str, out: &str, amt: u64) -> Option<Instruction> {
    let c = reqwest::Client::new();
    let q = c.get(QUOTE)
        .query(&[
            ("inputMint",       inp),
            ("outputMint",      out),
            ("amount",          &amt.to_string()),
            ("slippageBps",     "20"),
            ("onlyDirectRoutes","true"),
        ])
        .send().await.ok()?
        .json::<serde_json::Value>().await.ok()?;
    if q.get("error").is_some() { return None; }

    let s = c.post(SWAP)
        .json(&serde_json::json!({
            "quoteResponse":    q,
            "userPublicKey":    wallet,
            "wrapAndUnwrapSol": false,
        }))
        .send().await.ok()?
        .json::<serde_json::Value>().await.ok()?;
    if s.get("error").is_some() { return None; }

    let ix   = &s["swapInstruction"];
    let pid  = Pubkey::from_str(ix["programId"].as_str()?).ok()?;
    let data = general_purpose::STANDARD.decode(ix["data"].as_str()?).ok()?;
    let accs = ix["accounts"].as_array()?.iter().map(|a| {
        let pk = Pubkey::from_str(a["pubkey"].as_str().unwrap()).unwrap();
        if a["isWritable"].as_bool().unwrap_or(false) {
            AccountMeta::new(pk, a["isSigner"].as_bool().unwrap_or(false))
        } else {
            AccountMeta::new_readonly(pk, a["isSigner"].as_bool().unwrap_or(false))
        }
    }).collect();

    Some(Instruction { program_id: pid, accounts: accs, data })
}
