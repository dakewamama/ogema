use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;
use base64::{Engine as _, engine::general_purpose};
use jito_sdk_rust::JitoJsonRpcSDK;

const BURN_CU: &str = "3nmneSxGyY3vVLHe3b8pJtgYzmHCyihYe5ZFh8W9Nwhb";
const BURN_DISC: [u8; 8] = [0xd2, 0x3a, 0xa9, 0x2a, 0x04, 0xd0, 0xc9, 0x5e];

const JITO: &[&str] = &[
    "https://ny.mainnet.block-engine.jito.wtf/api/v1",
    "https://amsterdam.mainnet.block-engine.jito.wtf/api/v1",
    "https://frankfurt.mainnet.block-engine.jito.wtf/api/v1",
];

fn encode(tx: &Transaction) -> String {
    general_purpose::STANDARD.encode(bincode::serialize(tx).expect("serialize"))
}

pub fn build(kp: &Keypair, rpc: &RpcClient, ix2: Instruction, tip: u64) -> Vec<String> {
    let bh    = rpc.get_latest_blockhash().expect("blockhash");
    let payer = kp.pubkey();
    let tip_pk  = Pubkey::from_str(crate::tip::TIP_ACCT).unwrap();
    let burn_pk = Pubkey::from_str(BURN_CU).unwrap();

    let tx1 = Transaction::new_signed_with_payer(&[
        ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
        ComputeBudgetInstruction::set_compute_unit_price(1),
        Instruction {
            program_id: burn_pk,
            accounts:   vec![AccountMeta::new(payer, true)],
            data:       BURN_DISC.to_vec(),
        },
    ], Some(&payer), &[kp], bh);

    let tx2 = Transaction::new_signed_with_payer(&[ix2], Some(&payer), &[kp], bh);

    let tx3 = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(&payer, &tip_pk, tip)],
        Some(&payer), &[kp], bh,
    );

    vec![encode(&tx1), encode(&tx2), encode(&tx3)]
}

pub async fn send(bundle: Vec<String>) -> Option<(String, String)> {
    let futs: Vec<_> = JITO.iter().map(|ep| {
        let b  = bundle.clone();
        let ep = ep.to_string();
        tokio::spawn(async move {
            let r = JitoJsonRpcSDK::new(&ep, None)
                .send_bundle(Some(serde_json::json!(b)), None)
                .await
                .ok()?;
            r["result"].as_str().map(|id| (id.to_string(), ep))
        })
    }).collect();

    for f in futs {
        if let Ok(Some(result)) = f.await {
            return Some(result);
        }
    }
    None
}
