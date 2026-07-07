pub const TIP_ACCT: &str = "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL";
pub const TIP_MIN:  u64  = 10_000;
pub const TIP_MAX:  u64  = 500_000;

pub fn calc(gross_lamports: u64) -> u64 {
    (gross_lamports as f64 * 0.55) as u64
}

pub fn calc_usd(gross_usd: f64, sol_price: f64) -> u64 {
    let lamps = (gross_usd * 0.55 / sol_price * 1e9) as u64;
    lamps.clamp(TIP_MIN, TIP_MAX)
}
