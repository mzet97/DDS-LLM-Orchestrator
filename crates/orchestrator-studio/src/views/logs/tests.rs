use super::{fmt_offset, rel_offsets};
use crate::studio_log::LogEntry;

fn entry(offset_ms: u64) -> LogEntry {
    LogEntry {
        timestamp: String::from("00:00:00"),
        level: "INFO",
        source: "GUI",
        message: String::new(),
        payload: None,
        origin: String::new(),
        thread: String::new(),
        offset_ms,
        backtrace: None,
        slot: 0,
    }
}

#[test]
fn rel_offsets_anchor_newest_at_zero() {
    let entries = vec![entry(0), entry(2700), entry(100)];
    assert_eq!(rel_offsets(&entries), vec![-2800, -100, 0]);
    assert!(rel_offsets(&[]).is_empty());
}

#[test]
fn fmt_offset_matches_design_units() {
    assert_eq!(fmt_offset(0), "+0ms");
    assert_eq!(fmt_offset(-270), "-270ms");
    assert_eq!(fmt_offset(-2700), "-2.70s");
}
