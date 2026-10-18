use super::dispatch_history::{hhmmss_mmm, short_id};
use super::*;
use crate::workload::DispatchOutcome;

#[test]
fn clock_formats_with_millis() {
    assert_eq!(hhmmss_mmm(0), "00:00:00.000");
    assert_eq!(
        hhmmss_mmm(14 * 3_600_000 + 22 * 60_000 + 1_000 + 892),
        "14:22:01.892"
    );
    assert_eq!(hhmmss_mmm(86_400_000 + 5), "00:00:00.005");
}

#[test]
fn short_id_truncates_long_ids() {
    assert_eq!(short_id("t-1"), "t-1");
    assert_eq!(short_id("tsk_9c82f1b4a09"), "tsk_9c82f1b4…");
}

#[test]
fn conn_chip_never_probes() {
    let idle = DispatchState::new();
    assert_eq!(conn_chip(&idle).0, "○ NUNCA TESTADO");
    let mut unreachable = DispatchState::new();
    unreachable.result = String::from("erro de despacho: x");
    assert_eq!(conn_chip(&unreachable).0, "× ORQUESTRADOR INALCANÇÁVEL");
    let mut connected = DispatchState::new();
    connected.url = String::from("http://192.168.1.61:8080");
    connected.last = Some(DispatchOutcome::Failed {
        task_id: String::from("t"),
        error: String::from("e"),
    });
    assert_eq!(conn_chip(&connected).0, "× FALHA NO BACKEND");
}
