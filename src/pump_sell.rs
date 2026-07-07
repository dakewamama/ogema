use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_program;
use std::str::FromStr;

const PUMP_PROGRAM:  &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const FEE_PROGRAM:   &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM:   &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

const SELL_DISC: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
const FEE_CONFIG_SEED2: [u8; 32] = [
    1, 86, 224, 246, 147, 102, 90, 207, 68, 219, 21, 104, 191, 23, 91, 170,
    81, 137, 203, 151, 245, 210, 255, 59, 101, 93, 43, 182, 253, 109, 24, 176,
];

fn pk(s: &str) -> Pubkey { Pubkey::from_str(s).unwrap() }

pub struct SellAccounts {
    pub mint:          Pubkey,
    pub user:          Pubkey,
    pub creator:       Pubkey,
    pub fee_recipient: Pubkey,
}

pub fn build_sell(acc: &SellAccounts, amount: u64, min_sol_output: u64) -> Instruction {
    let program       = pk(PUMP_PROGRAM);
    let token_program = pk(TOKEN_PROGRAM);
    let ata_program   = pk(ATA_PROGRAM);

    let (global, _) = Pubkey::find_program_address(&[b"global"], &program);
    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", acc.mint.as_ref()], &program);
    let (associated_bonding_curve, _) = Pubkey::find_program_address(
        &[bonding_curve.as_ref(), token_program.as_ref(), acc.mint.as_ref()], &ata_program);
    let (associated_user, _) = Pubkey::find_program_address(
        &[acc.user.as_ref(), token_program.as_ref(), acc.mint.as_ref()], &ata_program);
    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", acc.creator.as_ref()], &program);
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &program);
    let (fee_config, _) = Pubkey::find_program_address(&[b"fee_config", &FEE_CONFIG_SEED2], &pk(FEE_PROGRAM));

    let mut data = Vec::with_capacity(24);
    data.extend_from_slice(&SELL_DISC);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&min_sol_output.to_le_bytes());

    let accounts = vec![
        AccountMeta::new_readonly(global, false),
        AccountMeta::new(acc.fee_recipient, false),
        AccountMeta::new_readonly(acc.mint, false),
        AccountMeta::new(bonding_curve, false),
        AccountMeta::new(associated_bonding_curve, false),
        AccountMeta::new(associated_user, false),
        AccountMeta::new(acc.user, true),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new(creator_vault, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(program, false),
        AccountMeta::new_readonly(fee_config, false),
        AccountMeta::new_readonly(pk(FEE_PROGRAM), false),
    ];

    Instruction { program_id: program, accounts, data }
}
