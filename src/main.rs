use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{read_keypair_file, Signer};
use jito_sdk_rust::JitoJsonRpcSDK;

#[tokio::main]
async fn main() {
    let kp = read_keypair_file("./keypair.json").unwrap();
    let rpc = RpcClient::new("https://api.mainnet-beta.solana.com".to_string());
    let bal = rpc.get_balance(&kp.pubkey()).unwrap();
    println!("wallet: {}", kp.pubkey());
    println!("balance: {} lamports", bal);

    let jito = JitoJsonRpcSDK::new("https://mainnet.block-engine.jito.wtf/api/v1", None);
    let tip_resp = jito.get_tip_accounts().await.unwrap();
    let tip_str = tip_resp["result"][0].as_str().unwrap();
    let tip_accounts = jito.get_tip_accounts().await.unwrap();

    println!("tip accounts: {:?}", tip_accounts["result"]);
}