mod geyser_trigger;
mod signal;

use signal::ArbSignal;
use std::io::Write;
use std::os::unix::net::UnixStream;
use tokio::sync::mpsc;

pub const SOCKET: &str = "/tmp/geyser_arb.sock";

fn push(sig: &ArbSignal) {
    match UnixStream::connect(SOCKET) {
        Ok(mut s) => {
            let _ = writeln!(
                s, "ARBSIGNAL {} {} {} {} {} {}",
                sig.pool_price, sig.pyth_price, sig.deviation, sig.direction,
                sig.reserve_in, sig.reserve_out
            );
        }
        Err(e) => eprintln!("geyser-watcher: ogema not ready: {e}"),
    }
}

async fn pyth_sol() -> f64 {
    let url = "https://hermes.pyth.network/v2/updates/price/latest?ids[]=0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";
    let Ok(resp) = reqwest::get(url).await else { return 0.0 };
    let Ok(data) = resp.json::<serde_json::Value>().await else { return 0.0 };
    let raw  = data["parsed"][0]["price"]["price"].as_str().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let expo = data["parsed"][0]["price"]["expo"].as_i64().unwrap_or(0);
    raw as f64 * 10f64.powi(expo as i32)
}

#[tokio::main]
async fn main() {
    let tatum_key = std::env::var("TATUM_KEY").expect("TATUM_KEY not set");
    let (tx, mut rx) = mpsc::channel::<ArbSignal>(32);

    let price = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    {
        let price = price.clone();
        tokio::spawn(async move {
            loop {
                let p = pyth_sol().await;
                if p > 0.0 { price.store(p.to_bits(), std::sync::atomic::Ordering::Relaxed); }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        });
    }

    {
        let price = price.clone();
        tokio::spawn(geyser_trigger::run(tx, tatum_key, move || {
            f64::from_bits(price.load(std::sync::atomic::Ordering::Relaxed))
        }));
    }

    eprintln!("geyser-watcher: forwarding signals to {SOCKET}");
    while let Some(sig) = rx.recv().await {
        push(&sig);
    }
}
