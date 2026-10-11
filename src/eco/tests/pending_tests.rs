use std::time::{Duration, Instant};

use crate::eco::comms::{Comms, Pending, PENDING_TIMEOUT, RETRY_COOLDOWN};

const MS: Duration = Duration::from_millis(1);

#[test]
fn a_fresh_state_may_act() {
    let mut c = Comms::new();
    assert!(c.may_act(Instant::now()));
}

#[test]
fn pending_request_blocks_until_the_timeout() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.start_neighbors(7, t0);
    assert!(!c.may_act(t0));
    assert!(!c.may_act(t0 + PENDING_TIMEOUT - MS));
}

#[test]
fn an_expired_request_is_dropped_and_followed_by_a_cooldown() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.start_travel(9, t0);
    let expiry = t0 + PENDING_TIMEOUT;
    assert!(!c.may_act(expiry)); // expired -> cooldown starts
    assert_eq!(c.pending, Pending::Idle);
    assert!(!c.may_act(expiry + RETRY_COOLDOWN - MS));
    assert!(c.may_act(expiry + RETRY_COOLDOWN));
}

#[test]
fn a_matching_reply_clears_pending_immediately() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.start_neighbors(7, t0);
    c.resolve_neighbors();
    assert!(c.may_act(t0));
}

#[test]
fn a_reply_of_the_other_kind_does_not_clear_pending() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.start_neighbors(7, t0);
    c.resolve_travel();
    assert!(!c.may_act(t0));

    let mut c = Comms::new();
    c.start_travel(9, t0);
    c.resolve_neighbors();
    assert!(!c.may_act(t0));
}

#[test]
fn cool_down_blocks_for_the_cooldown_only() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.cool_down(t0);
    assert!(!c.may_act(t0));
    assert!(!c.may_act(t0 + RETRY_COOLDOWN - MS));
    assert!(c.may_act(t0 + RETRY_COOLDOWN));
}

#[test]
fn a_failed_probe_blocks_probing_but_not_acting() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    assert!(c.may_probe(t0));
    c.probe_failed(t0);
    assert!(!c.may_probe(t0));
    assert!(!c.may_probe(t0 + RETRY_COOLDOWN - MS));
    assert!(c.may_probe(t0 + RETRY_COOLDOWN));
    assert!(c.may_act(t0)); // a failed probe never stops Eco from acting
}

#[test]
fn resolving_neighbors_returns_the_planet_that_was_asked() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.start_neighbors(7, t0);
    assert_eq!(c.resolve_neighbors(), Some(7));
    assert_eq!(c.resolve_neighbors(), None); // nothing pending any more
}

#[test]
fn travel_target_is_set_only_while_a_travel_is_pending() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    assert_eq!(c.travel_target(), None);
    c.start_travel(9, t0);
    assert_eq!(c.travel_target(), Some(9));
    c.resolve_travel();
    assert_eq!(c.travel_target(), None);
}

#[test]
fn an_expired_request_is_reported_once() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.start_travel(9, t0);
    c.may_act(t0 + PENDING_TIMEOUT);
    assert!(matches!(c.take_expired(), Some(Pending::Travel { to: 9, .. })));
    assert_eq!(c.take_expired(), None);
}

#[test]
fn reset_forgets_pending_requests_and_cooldowns() {
    let t0 = Instant::now();
    let mut c = Comms::new();
    c.start_neighbors(7, t0);
    c.cool_down(t0);
    c.probe_failed(t0);
    c.reset();
    assert!(c.may_act(t0));
    assert!(c.may_probe(t0));
}