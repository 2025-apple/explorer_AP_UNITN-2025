//! Eco's coin balance: charge, credit, and a debt check. Deliberately
//! tiny and dependency-free so it's trivial to reason about in isolation.

#[derive(Debug, Clone, Copy)]
pub struct Wallet {
    pub coins: i64,
}

impl Wallet {
    pub fn new(starting: i64) -> Self {
        Self { coins: starting }
    }

    pub fn charge(&mut self, amount: u32) {
        self.coins -= amount as i64;
    }

    pub fn credit(&mut self, amount: u32) {
        self.coins += amount as i64;
    }

    pub fn is_in_debt(&self) -> bool {
        self.coins < 0
    }
}