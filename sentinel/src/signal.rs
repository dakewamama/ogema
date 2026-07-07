use std::io::Write;
use std::os::unix::net::UnixStream;

pub const SOCKET: &str = "/tmp/sentinel.sock";

pub fn fire(pubkey: &str, lamports: u64, slot: u64) {
    match UnixStream::connect(SOCKET) {
        Ok(mut s) => { let _ = writeln!(s, "SIGNAL {} {} {}", pubkey, lamports, slot); }
        Err(e) => eprintln!("sentinel: ogema not ready: {e}"),
    }
}
