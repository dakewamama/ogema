use async_trait::async_trait;

#[async_trait]
pub trait SearchGoal<Ctx, Input, Output>: Send + Sync {
    async fn evaluate(&self, input: Input, ctx: &Ctx) -> (u64, Output);
}

const PHI_COMPLEMENT: f64 = 0.3819660112501051;
const GOLDEN_RATIO:   f64 = 0.6180339887498949;
const MAX_ITERS:      u32 = 24;
const MIN_INTERVAL:   u64 = 1_000;

pub async fn maximize<Ctx, Goal, Output>(lo: u64, hi: u64, goal: Goal, ctx: &Ctx) -> (u64, u64, Output)
where
    Ctx: Sync, Goal: SearchGoal<Ctx, u64, Output>, Output: Default,
{
    let mut lo = lo;
    let mut hi = hi.max(lo + MIN_INTERVAL);
    let mut x1 = lo + ((hi - lo) as f64 * PHI_COMPLEMENT) as u64;
    let mut x2 = lo + ((hi - lo) as f64 * GOLDEN_RATIO) as u64;

    let (mut f1, mut o1) = goal.evaluate(x1, ctx).await;
    let (mut f2, mut o2) = goal.evaluate(x2, ctx).await;

    let mut best_x = if f1 >= f2 { x1 } else { x2 };
    let mut best_f = f1.max(f2);
    let mut best_o = if f1 >= f2 { std::mem::take(&mut o1) } else { std::mem::take(&mut o2) };

    for _ in 0..MAX_ITERS {
        if hi - lo < MIN_INTERVAL { break; }
        if f1 >= f2 {
            hi = x2; x2 = x1; f2 = f1;
            x1 = lo + ((hi - lo) as f64 * PHI_COMPLEMENT) as u64;
            (f1, o1) = goal.evaluate(x1, ctx).await;
        } else {
            lo = x1; x1 = x2; f1 = f2;
            x2 = lo + ((hi - lo) as f64 * GOLDEN_RATIO) as u64;
            (f2, o2) = goal.evaluate(x2, ctx).await;
        }
        let (rx, rf) = if f1 >= f2 { (x1, f1) } else { (x2, f2) };
        if rf > best_f {
            best_f = rf; best_x = rx;
            best_o = if f1 >= f2 { std::mem::take(&mut o1) } else { std::mem::take(&mut o2) };
        }
    }
    (best_x, best_f, best_o)
}
