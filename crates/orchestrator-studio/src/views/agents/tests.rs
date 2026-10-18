use super::{hhmmss_ms, success_rate};

#[test]
fn polling_clock_formats_utc_with_millis() {
    assert_eq!(hhmmss_ms(0), "00:00:00.000");
    assert_eq!(hhmmss_ms(3_661_005), "01:01:01.005");
}

#[test]
fn success_rate_needs_completed_or_failed() {
    assert_eq!(success_rate(0, 0), None);
    assert!((success_rate(3, 1).unwrap() - 75.0).abs() < f64::EPSILON);
}
