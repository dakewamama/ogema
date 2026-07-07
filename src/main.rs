mod bundle;
mod geyser_trigger;
mod jup;
mod puffin;
mod pyth;
mod pyth_shared;
mod signal;
mod sentinel_receiver;
mod tip;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{signature::{read_keypair_file, Signer}};
use tokio::time::{sleep, Duration};

const RPC_URL:    &str = "https://mainnet.helius-rpc.com/?api-key=YOUR_HELIUS_KEY";
const USDC:       &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDT:       &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
const THRESHOLD:  f64  = 0.001;
const SWAP_AMT:   u64  = 1_000_000;
const REFRESH_SEC:u64  = 3600;

#[tokio::main]
async fn main() {
    let kp  = read_keypair_file("./keypair.json").unwrap();
    let rpc = RpcClient::new(RPC_URL.to_string());
    let w   = kp.pubkey().to_string();
    eprintln!("{} bal={}", w, rpc.get_balance(&kp.pubkey()).unwrap());

    let mut delays    = puffin::load().await;
    let mut refreshed = std::time::Instant::now();

    let (sentinel_tx, mut sentinel_rx) = tokio::sync::mpsc::channel::<sentinel_receiver::Signal>(256);
    tokio::spawn(sentinel_receiver::run(sentinel_tx));

    let (arb_tx, mut arb_rx) = tokio::sync::mpsc::channel::<signal::ArbSignal>(32);
    let tatum_key = std::env::var("TATUM_KEY").unwrap_or_default();
    let sol_price_shared = pyth_shared::SharedPrice::new();
    {
        let shared = sol_price_shared.clone();
        tokio::spawn(geyser_trigger::run(arb_tx, tatum_key, move || shared.get()));
    }

    loop {
        if refreshed.elapsed().as_secs() > REFRESH_SEC {
            delays    = puffin::load().await;
            refreshed = std::time::Instant::now();
        }

        let slot = rpc.get_slot().unwrap_or(0);
        let leaders: Vec<String> = rpc.get_slot_leaders(slot, 8)
            .unwrap_or_default().iter().map(|p| p.to_string()).collect();
        let (ok, why) = puffin::check(&leaders, &delays);

        let p = pyth::prices().await;
        if p.is_empty() { sleep(Duration::from_millis(200)).await; continue; }

        let get = |target: &str| p.iter().find(|(id,_)| id.ends_with(target)).map(|(_,v)| *v).unwrap_or(0.0);
        let usdc = get("94a");
        let usdt = get("53b");
        let sol  = get("56d");
        let spread = (usdc - usdt).abs();
        if sol > 0.0 { sol_price_shared.set(sol); }

        println!("slot={slot} usdc={usdc:.6} usdt={usdt:.6} spread={spread:.6} leader={why} ok={ok}");

        if spread > THRESHOLD && ok {
            let (inp, out) = if usdc > usdt { (USDC, USDT) } else { (USDT, USDC) };
            let gross_usd = SWAP_AMT as f64 / 1e6 * spread;
            let tip = tip::calc_usd(gross_usd, usdc.max(1.0));
            match jup::swap_ix(&w, inp, out, SWAP_AMT).await {
                Some(ix) => match bundle::send(bundle::build(&kp, &rpc, ix, tip)).await {
                    Some((id, ep)) => println!("fired {ep} tip={tip} | {id}"),
                    None           => eprintln!("no endpoint"),
                },
                None => eprintln!("jup failed"),
            }
        }

        if ok {
            if let Ok(sig) = arb_rx.try_recv() {
                eprintln!("arb signal: dev={:.4}% dir={}", sig.deviation * 100.0, sig.direction);
            }
        }

        if ok {
            if let Ok(sig) = sentinel_rx.try_recv() {
                eprintln!("sentinel signal: wallet={} lamports={} slot={}", sig.pubkey, sig.lamports, sig.slot);
            }
        }

        sleep(Duration::from_millis(400)).await;
    }
}
