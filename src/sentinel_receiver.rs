use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::Sender;

pub const SOCKET: &str = "/tmp/sentinel.sock";

#[derive(Debug, Clone)]
pub struct Signal { pub pubkey: String, pub lamports: u64, pub slot: u64 }

fn parse(line: &str) -> Option<Signal> {
    let mut p = line.trim().split_ascii_whitespace();
    (p.next()? == "SIGNAL").then_some(())?;
    Some(Signal {
        pubkey:   p.next()?.to_string(),
        lamports: p.next()?.parse().ok()?,
        slot:     p.next()?.parse().ok()?,
    })
}

async fn handle(stream: UnixStream, tx: Sender<Signal>) {
    let mut lines = BufReader::new(stream).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => match parse(&line) {
                Some(s) => { if tx.send(s).await.is_err() { return; } }
                None    => eprintln!("sentinel_receiver: unparseable: {line}"),
            },
            Ok(None) | Err(_) => return,
        }
    }
}

pub async fn run(tx: Sender<Signal>) {
    let _ = std::fs::remove_file(SOCKET);
    let listener = match UnixListener::bind(SOCKET) {
        Ok(l)  => l,
        Err(e) => { eprintln!("sentinel_receiver: bind {e}"); return; }
    };
    eprintln!("sentinel_receiver: {SOCKET}");
    loop {
        match listener.accept().await {
            Ok((stream, _)) => { tokio::spawn(handle(stream, tx.clone())); }
            Err(e)          => eprintln!("sentinel_receiver: accept {e}"),
        }
    }
}
