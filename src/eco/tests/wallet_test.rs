use crate::eco::wallet::Wallet;

#[test]
fn charge_and_credit_move_the_balance() {
    let mut w = Wallet::new(120);
    w.charge(10);
    assert_eq!(w.coins, 110);
    w.credit(25);
    assert_eq!(w.coins, 135);
}

#[test]
fn debt_starts_below_zero() {
    let mut w = Wallet::new(5);
    w.charge(5);
    assert!(!w.is_in_debt()); // exactly zero is not debt
    w.charge(1);
    assert!(w.is_in_debt());
    assert_eq!(w.coins, -1);
}