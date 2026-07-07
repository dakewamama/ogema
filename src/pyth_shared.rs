use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct SharedPrice(Arc<AtomicU64>);

impl SharedPrice {
    pub fn new() -> Self { Self(Arc::new(AtomicU64::new(0))) }
    pub fn set(&self, price: f64) { self.0.store(price.to_bits(), Ordering::Relaxed); }
    pub fn get(&self) -> f64 { f64::from_bits(self.0.load(Ordering::Relaxed)) }
}
