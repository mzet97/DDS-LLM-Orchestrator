use super::*;

#[test]
fn clock_ms_matches_design() {
    assert_eq!(clock_ms(0), "00:00:00.000");
    assert_eq!(clock_ms(50_569_102), "14:02:49.102");
}
