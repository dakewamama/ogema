use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{read_keypair_file, Signer};

fn main() {
    let kp = read_keypair_file("./keypair.json").unwrap();
    let rpc = RpcClient::new("https://api.mainnet-beta.solana.com".to_string());
    let bal = rpc.get_balance(&kp.pubkey()).unwrap();
    println!("wallet: {}", kp.pubkey());
    println!("balance: {} lamports", bal);
}
