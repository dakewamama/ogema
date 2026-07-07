#[derive(Debug, Clone)]
pub struct ArbSignal {
    pub pool_price:  f64,
    pub pyth_price:  f64,
    pub deviation:   f64,
    pub direction:   i8,
    pub reserve_in:  u64,
    pub reserve_out: u64,
    pub fair_price:  f64,
}
