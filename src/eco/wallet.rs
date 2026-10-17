//! Eco's coin balance: charge, credit, and a debt check. Deliberately
//! tiny and dependency-free so it's trivial to reason about in isolation.
//! It also keeps two running totals (earned and spent) so the status
//! panel can explain where the coins went.

#[derive(Debug, Clone, Copy)]
pub struct Wallet {
    pub coins: i64,
    /// Total income credited since the wallet was created.
    pub earned: i64,
    /// Total charged for actions since the wallet was created.
    pub spent: i64,
}

impl Wallet {
    pub fn new(starting: i64) -> Self {
        Self {
            coins: starting,
            earned: 0,
            spent: 0,
        }
    }

    pub fn charge(&mut self, amount: u32) {
        self.coins -= i64::from(amount);
        self.spent += i64::from(amount);
    }

    pub fn credit(&mut self, amount: u32) {
        self.coins += i64::from(amount);
        self.earned += i64::from(amount);
    }

    pub fn is_in_debt(&self) -> bool {
        self.coins < 0
    }
}
