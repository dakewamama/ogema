use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{read_keypair_file, Signer};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::instruction::{Instruction, AccountMeta};
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use jito_sdk_rust::JitoJsonRpcSDK;
use std::str::FromStr;

const BURN_CU_PROGRAM: &str = "3nmneSxGyY3vVLHe3b8pJtgYzmHCyihYe5ZFh8W9Nwhb";

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
    let tip_account = Pubkey::from_str(tip_str).unwrap();

    let blockhash = rpc.get_latest_blockhash().unwrap();
    let payer = kp.pubkey();

    let burn_prog = Pubkey::from_str(BURN_CU_PROGRAM).unwrap();
    let burn_ix = Instruction {
        program_id: burn_prog,
        accounts: vec![AccountMeta::new(payer, true)],
        data: vec![0x6e, 0x4a, 0x8d, 0x2c, 0x1b, 0x9f, 0x3e, 0x7a],
    };
    let tx1 = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            ComputeBudgetInstruction::set_compute_unit_price(1),
            burn_ix,
        ],
        Some(&payer), &[&kp], blockhash,
    );

    let tx2 = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(&payer, &payer, 0)],
        Some(&payer), &[&kp], blockhash,
    );

    let tx3 = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(&payer, &tip_account, 10_000)],
        Some(&payer), &[&kp], blockhash,
    );

    let encode = |tx: &Transaction| -> String {
        bs58::encode(bincode::serialize(tx).unwrap()).into_string()
    };

    let bundle = vec![encode(&tx1), encode(&tx2), encode(&tx3)];
    let result = jito.send_bundle(Some(serde_json::json!(bundle)), None).await.unwrap();
    println!("response: {:?}", result);
}