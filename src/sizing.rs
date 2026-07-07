use std::sync::Arc;
use anyhow::{ensure, Result};
use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use tokio::task::JoinSet;
use crate::search::{maximize, SearchGoal};
use crate::signal::ArbSignal;

const GRID_START: u64 = 100_000_000;
const GRID_STEPS: u32 = 6;
const GSS_SPAN:   u64 = 10;
const FEE_BPS:    u64 = 25;

pub struct LocalAmm;

#[async_trait]
pub trait Simulator: Send + Sync {
    async fn profit(&self, signal: &ArbSignal, sender: Pubkey, amount_in: u64) -> Result<i64>;
}

#[async_trait]
impl Simulator for LocalAmm {
    async fn profit(&self, s: &ArbSignal, _: Pubkey, amount_in: u64) -> Result<i64> {
        ensure!(s.reserve_in > 0 && s.reserve_out > 0, "empty reserves");
        ensure!(s.fair_price > 0.0, "invalid fair_price");
        let after_fee  = amount_in.saturating_mul(10_000 - FEE_BPS) / 10_000;
        let amount_out = ((s.reserve_out as u128) * (after_fee as u128))
            / ((s.reserve_in as u128) + (after_fee as u128));
        Ok((amount_out as f64 * s.fair_price - amount_in as f64) as i64)
    }
}

#[derive(Debug, Default, Clone)]
pub struct Trial { pub amount_in: u64, pub profit: i64 }

impl PartialEq  for Trial { fn eq(&self, o: &Self) -> bool { self.profit == o.profit } }
impl Eq         for Trial {}
impl PartialOrd for Trial { fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { self.profit.partial_cmp(&o.profit) } }
impl Ord        for Trial { fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.profit.cmp(&o.profit) } }

struct Ctx { signal: ArbSignal, sender: Pubkey, sim: Arc<dyn Simulator> }

impl Ctx {
    async fn trial(&self, amount_in: u64) -> Trial {
        match self.sim.profit(&self.signal, self.sender, amount_in).await {
            Ok(p) if p > 0 => Trial { amount_in, profit: p },
            _              => Trial::default(),
        }
    }
}

struct Goal;

#[async_trait]
impl SearchGoal<Ctx, u64, Trial> for Goal {
    async fn evaluate(&self, amount_in: u64, ctx: &Ctx) -> (u64, Trial) {
        let t = ctx.trial(amount_in).await;
        (t.profit.max(0) as u64, t)
    }
}

pub async fn optimal(signal: ArbSignal, sender: Pubkey, sim: Arc<dyn Simulator>) -> Result<Trial> {
    let ctx = Arc::new(Ctx { signal, sender, sim });
    let mut js = JoinSet::new();
    for step in 0..GRID_STEPS {
        let ctx = ctx.clone();
        let amt = GRID_START.saturating_mul(10u64.pow(step));
        js.spawn(async move { ctx.trial(amt).await });
    }
    let mut best = Trial::default();
    while let Some(Ok(t)) = js.join_next().await { if t > best { best = t; } }
    ensure!(best.profit > 0, "no profitable grid point");
    let (_, _, gss) = maximize(best.amount_in / GSS_SPAN, best.amount_in.saturating_mul(GSS_SPAN), Goal, &*ctx).await;
    ensure!(gss.profit > 0, "no profitable size after gss");
    Ok(gss.max(best))
}
