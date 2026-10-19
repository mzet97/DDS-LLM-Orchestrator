use super::*;

#[test]
fn chrono_now_reads_unix_nanos() {
    // 2026-10-08T09:00:00Z = 1791450000 s (`date -u`).
    assert_eq!(chrono_now(1_791_450_000_000_000_000), "09:00:00");
    assert_eq!(chrono_now(0), "00:00:00");
}

#[test]
fn push_caps_the_buffer_and_keeps_order() {
    // Prefixo único: testes do binary rodam em paralelo no mesmo buffer.
    let prefix = format!("evt-{}-", std::process::id());
    for i in 0..(CAPACITY + 50) {
        super::push(
            "INFO",
            "TEST",
            None,
            format!("{prefix}{i}"),
            String::from("t"),
        );
    }
    let all = entries();
    let mine: Vec<&LogEntry> = all
        .iter()
        .filter(|entry| entry.message.starts_with(&prefix))
        .collect();
    assert_eq!(mine.len(), CAPACITY, "buffer limitado em {CAPACITY}");
    // Ordem preservada: sobrevivem 50..(CAPACITY+50), ascendente.
    assert_eq!(mine[0].message, format!("{prefix}50"));
    assert_eq!(
        mine.last().expect("nonempty").message,
        format!("{prefix}{}", CAPACITY + 49)
    );
}

/// PRD 3.14: subsistema/payload/fonte/thread/offset/slot atravessam.
/// (marcador único: os testes do binary rodam em paralelo e compartilham
/// o buffer estático — índice absoluto seria flaky.)
#[test]
fn tagged_entry_carries_inspector_fields() {
    let marker = format!("falha-inspetor-{}", std::process::id());
    super::push(
        "ERRO",
        "SYSTEMD",
        Some(String::from(r#"{"unit":"llama-server"}"#)),
        marker.clone(),
        String::from("services.rs:120"),
    );
    let all = entries();
    let entry = all
        .iter()
        .rev()
        .find(|entry| entry.message == marker)
        .expect("entrada marcada appended");
    assert_eq!(entry.source, "SYSTEMD");
    assert_eq!(entry.payload.as_deref(), Some(r#"{"unit":"llama-server"}"#));
    assert_eq!(entry.origin, "services.rs:120");
    assert!(entry.thread.starts_with("ThreadId("));
    assert!(entry.backtrace.is_some(), "ERRO captura stack trace");
    assert!(entry.slot > 0);
}
