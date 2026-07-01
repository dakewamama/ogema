use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{read_keypair_file, Signer};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::instruction::{Instruction, AccountMeta};
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use jito_sdk_rust::JitoJsonRpcSDK;
use std::str::FromStr;
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose};
use tokio::time::{sleep, Duration};

const BURN_CU:    &str = "3nmneSxGyY3vVLHe3b8pJtgYzmHCyihYe5ZFh8W9Nwhb";
const TIP:        &str = "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL";
const RPC_URL:    &str = "https://mainnet.helius-rpc.com/?api-key=545abf5d-5eda-4b05-ae87-b3cb1a4a2643";
const PUFFIN_URL: &str = "https://puffin-validator.xyz/inclusion_delays";
const PYTH_URL:   &str = "https://hermes.pyth.network/v2/updates/price/latest";
const JUP_QUOTE:  &str = "https://quote-api.jup.ag/v6/quote";
const JUP_SWAP:   &str = "https://quote-api.jup.ag/v6/swap-instructions";
const USDC:       &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDT:       &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

const JITO: &[&str] = &[
    "https://ny.mainnet.block-engine.jito.wtf/api/v1",
    "https://amsterdam.mainnet.block-engine.jito.wtf/api/v1",
    "https://frankfurt.mainnet.block-engine.jito.wtf/api/v1",
];

const FEEDS: [(&str, &str); 4] = [
    ("SOL",  "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"),
    ("USDC", "0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a"),
    ("USDT", "0x2b89b9dc8fdf9f34709a5b106b472f0f39bb6ca9ce04b0fd7f2e971688e2e53b"),
    ("BONK", "0x72b021217ca3fe68922a19aaf990109cb9d84e9ad004b4d2025ad6f529314419"),
];

const MAX_DELAY:   u64   = 150;
const THRESHOLD:   f64   = 0.001;
const SWAP_AMT:    u64   = 1_000_000;
const TIP_LAMPS:   u64   = 10_000;
const REFRESH_SEC: u64   = 3600;

struct VDelay { ms: u64, client: String, order: u8 }

async fn load_delays() -> HashMap<String, VDelay> {
    let html = match reqwest::get(PUFFIN_URL).await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    let row = regex::Regex::new(r"<tr[^>]*>(.*?)</tr>").unwrap();
    let pk  = regex::Regex::new(r"<!--node-id\d+-->([1-9A-HJ-NP-Za-km-z]{32,44})<!--#-->").unwrap();
    let num = regex::Regex::new(r"<!--node-id\d+-->(\d+)<!--#-->").unwrap();
    let ord = regex::Regex::new(r"<!--node-id\d+-->(\d+)\s*%<!--#-->").unwrap();
    let cli = regex::Regex::new(r"<!--node-id\d+-->(Jito|Agave|Firedancer|Frankendancer|Rakurai|BAM|Harmonic[^<]*)<!--#-->").unwrap();
    for cap in row.captures_iter(&html) {
        let r = &cap[1];
        let pks: Vec<&str> = pk.captures_iter(r).map(|c| c.get(1).unwrap().as_str()).collect();
        if pks.is_empty() { continue; }
        let nums: Vec<u64> = num.captures_iter(r).filter_map(|c| c[1].parse().ok()).collect();
        if nums.len() < 2 { continue; }
        out.insert(pks[0].to_string(), VDelay {
            ms: nums[1],
            client: cli.captures(r).map(|c| c[1].to_string()).unwrap_or_default(),
            order: ord.captures(r).and_then(|c| c[1].parse().ok()).unwrap_or(0),
        });
    }
    eprintln!("delays: {} validators", out.len());
    out
}

fn check_leader(leaders: &[String], delays: &HashMap<String, VDelay>) -> (bool, String) {
    for l in leaders.iter().take(4) {
        return match delays.get(l) {
            Some(d) if d.ms <= MAX_DELAY => (true,  format!("{}ms {} {}%", d.ms, d.client, d.order)),
            Some(d)                       => (false, format!("slow {}ms {}", d.ms, d.client)),
            None                          => (true,  "unknown".into()),
        };
    }
    (false, "no leaders".into())
}

async fn pyth_prices() -> Vec<(&'static str, f64)> {
    let ids = FEEDS.iter().map(|(_, id)| format!("ids[]={}", id)).collect::<Vec<_>>().join("&");
    let data: serde_json::Value = match reqwest::get(format!("{}?{}", PYTH_URL, ids)).await {
        Ok(r) => r.json().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    data["parsed"].as_array().unwrap_or(&vec![]).iter().map(|p| {
        let raw  = p["price"]["price"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
        let expo = p["price"]["expo"].as_i64().unwrap_or(0);
        let val  = raw as f64 * 10f64.powi(expo as i32);
        let fid  = format!("0x{}", p["id"].as_str().unwrap_or(""));
        let sym  = FEEDS.iter().find(|(_, f)| *f == fid).map(|(s, _)| *s).unwrap_or("?");
        (sym, val)
    }).collect()
}

async fn swap_ix(wallet: &str, inp: &str, out: &str, amt: u64) -> Option<Vec<Instruction>> {
    let c = reqwest::Client::new();
    let q = c.get(JUP_QUOTE)
        .query(&[("inputMint",inp),("outputMint",out),("amount",&amt.to_string()),("slippageBps","20"),("onlyDirectRoutes","true")])
        .send().await.ok()?.json::<serde_json::Value>().await.ok()?;
    if q.get("error").is_some() { return None; }
    let s = c.post(JUP_SWAP)
        .json(&serde_json::json!({"quoteResponse":q,"userPublicKey":wallet,"wrapAndUnwrapSol":false}))
        .send().await.ok()?.json::<serde_json::Value>().await.ok()?;
    if s.get("error").is_some() { return None; }
    let ix = &s["swapInstruction"];
    let pid = Pubkey::from_str(ix["programId"].as_str()?).ok()?;
    let accs: Vec<AccountMeta> = ix["accounts"].as_array()?.iter().map(|a| {
        let pk = Pubkey::from_str(a["pubkey"].as_str().unwrap()).unwrap();
        if a["isWritable"].as_bool().unwrap_or(false) { AccountMeta::new(pk, a["isSigner"].as_bool().unwrap_or(false)) }
        else { AccountMeta::new_readonly(pk, a["isSigner"].as_bool().unwrap_or(false)) }
    }).collect();
    let data = general_purpose::STANDARD.decode(ix["data"].as_str()?).ok()?;
    Some(vec![Instruction { program_id: pid, accounts: accs, data }])
}

fn mk_bundle(kp: &solana_sdk::signature::Keypair, rpc: &RpcClient, ixs: Vec<Instruction>) -> Vec<String> {
    let bh    = rpc.get_latest_blockhash().unwrap();
    let payer = kp.pubkey();
    let tx1 = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            ComputeBudgetInstruction::set_compute_unit_price(1),
            Instruction {
                program_id: Pubkey::from_str(BURN_CU).unwrap(),
                accounts: vec![AccountMeta::new(payer, true)],
                data: vec![0xd2, 0x3a, 0xa9, 0x2a, 0x04, 0xd0, 0xc9, 0x5e],
            },
        ],
        Some(&payer), &[kp], bh,
    );
    let tx2 = Transaction::new_signed_with_payer(&ixs, Some(&payer), &[kp], bh);
    let tx3 = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(&payer, &Pubkey::from_str(TIP).unwrap(), TIP_LAMPS)],
        Some(&payer), &[kp], bh,
    );
    let enc = |tx: &Transaction| general_purpose::STANDARD.encode(bincode::serialize(tx).unwrap());
    vec![enc(&tx1), enc(&tx2), enc(&tx3)]
}

async fn send(bundle: Vec<String>) -> Option<String> {
    let futs: Vec<_> = JITO.iter().map(|ep| {
        let b  = bundle.clone();
        let ep = ep.to_string();
        tokio::spawn(async move {
            let r = JitoJsonRpcSDK::new(&ep, None).send_bundle(Some(serde_json::json!(b)), None).await.ok()?;
            r["result"].as_str().map(|s| s.to_string())
        })
    }).collect();
    for f in futs { if let Ok(Some(id)) = f.await { return Some(id); } }
    None
}

#[tokio::main]
async fn main() {
    let kp  = read_keypair_file("./keypair.json").unwrap();
    let rpc = RpcClient::new(RPC_URL.to_string());
    let w   = kp.pubkey().to_string();
    eprintln!("{} bal={}", w, rpc.get_balance(&kp.pubkey()).unwrap());

    let mut delays      = load_delays().await;
    let mut refreshed   = std::time::Instant::now();

    loop {
        if refreshed.elapsed().as_secs() > REFRESH_SEC {
            delays    = load_delays().await;
            refreshed = std::time::Instant::now();
        }

        let slot    = rpc.get_slot().unwrap_or(0);
        let leaders: Vec<String> = rpc.get_slot_leaders(slot, 8).unwrap_or_default()
            .iter().map(|p| p.to_string()).collect();
        let (ok, why) = check_leader(&leaders, &delays);

        let p = pyth_prices().await;
        if p.is_empty() { sleep(Duration::from_millis(200)).await; continue; }

        let usdc   = p.iter().find(|(s,_)| *s=="USDC").map(|(_,v)| *v).unwrap_or(0.0);
        let usdt   = p.iter().find(|(s,_)| *s=="USDT").map(|(_,v)| *v).unwrap_or(0.0);
        let sol    = p.iter().find(|(s,_)| *s=="SOL").map(|(_,v)| *v).unwrap_or(0.0);
        let spread = (usdc - usdt).abs();

        println!("{} sol={:.2} usdc={:.6} usdt={:.6} spread={:.6} leader={} ok={}",
            slot, sol, usdc, usdt, spread, why, ok);

        if spread > THRESHOLD && ok {
            let (inp, out) = if usdc > usdt { (USDC, USDT) } else { (USDT, USDC) };
            match swap_ix(&w, inp, out, SWAP_AMT).await {
                Some(ixs) => match send(mk_bundle(&kp, &rpc, ixs)).await {
                    Some(id) => println!("fired {id}"),
                    None     => eprintln!("no endpoint"),
                },
                None => eprintln!("jup failed"),
            }
        }

        sleep(Duration::from_millis(400)).await;
    }
}