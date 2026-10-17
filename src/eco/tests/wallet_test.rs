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

#[test]
fn earned_and_spent_explain_the_balance() {
    let mut w = Wallet::new(18);
    w.charge(4);
    w.credit(12);
    w.charge(5);
    assert_eq!(w.earned, 12);
    assert_eq!(w.spent, 9);
    assert_eq!(w.coins, 18 + w.earned - w.spent);
}
