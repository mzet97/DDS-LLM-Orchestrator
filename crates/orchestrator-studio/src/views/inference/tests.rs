use super::{hhmmss_utc, session_tokens, temp_caption, temp_mode};
use crate::inference::{InferenceState, TurnStats};

#[test]
fn clock_formats_utc_wall_time() {
    assert_eq!(hhmmss_utc(0), "00:00:00 UTC");
    assert_eq!(hhmmss_utc(3_661_000), "01:01:01 UTC");
    assert_eq!(hhmmss_utc(86_399_999), "23:59:59 UTC");
}

#[test]
fn temperature_buckets_match_documented_ranges() {
    assert_eq!(temp_caption(0.2), "Preciso / baixo jitter");
    assert_eq!(temp_caption(0.5), "Equilibrado");
    assert_eq!(temp_caption(1.5), "Criativo / dispersivo");
    assert_eq!(temp_mode(0.0), "DETERMINÍSTICO");
    assert_eq!(temp_mode(0.2), "FOCADO");
    assert_eq!(temp_mode(1.0), "EXPLORATÓRIO");
}

#[test]
fn session_tokens_sum_known_usage_only() {
    let mut infer = InferenceState::new();
    assert_eq!(session_tokens(&infer), (0, 0));
    infer.stats.push(TurnStats {
        elapsed_ms: 100,
        prompt_tokens: Some(10),
        completion_tokens: Some(5),
    });
    infer.stats.push(TurnStats {
        elapsed_ms: 50,
        prompt_tokens: None,
        completion_tokens: None,
    });
    assert_eq!(session_tokens(&infer), (15, 2));
}
