use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::Sender;

use crate::signal::ArbSignal;

pub const SOCKET: &str = "/tmp/geyser_arb.sock";

fn parse(line: &str) -> Option<ArbSignal> {
    let mut p = line.trim().split_ascii_whitespace();
    if p.next()? != "ARBSIGNAL" { return None; }
    let pool_price:  f64 = p.next()?.parse().ok()?;
    let pyth_price:  f64 = p.next()?.parse().ok()?;
    let deviation:   f64 = p.next()?.parse().ok()?;
    let direction:   i8  = p.next()?.parse().ok()?;
    let reserve_in:  u64 = p.next()?.parse().ok()?;
    let reserve_out: u64 = p.next()?.parse().ok()?;
    Some(ArbSignal { pool_price, pyth_price, deviation, direction, reserve_in, reserve_out, fair_price: pyth_price })
}

async fn handle(stream: UnixStream, tx: Sender<ArbSignal>) {
    let mut lines = BufReader::new(stream).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => match parse(&line) {
                Some(s) => { if tx.send(s).await.is_err() { return; } }
                None    => eprintln!("arb_receiver: unparseable: {line}"),
            },
            Ok(None) | Err(_) => return,
        }
    }
}

pub async fn run(tx: Sender<ArbSignal>) {
    let _ = std::fs::remove_file(SOCKET);
    let listener = match UnixListener::bind(SOCKET) {
        Ok(l)  => l,
        Err(e) => { eprintln!("arb_receiver: bind {e}"); return; }
    };
    eprintln!("arb_receiver: {SOCKET}");
    loop {
        match listener.accept().await {
            Ok((stream, _)) => { tokio::spawn(handle(stream, tx.clone())); }
            Err(e)          => eprintln!("arb_receiver: accept {e}"),
        }
    }
}
