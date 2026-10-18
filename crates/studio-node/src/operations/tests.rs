use super::*;

fn log_with(service: &str) -> OperationLog {
    OperationLog::new(vec![String::from(service)])
}

fn op_id(id: &str) -> OperationId {
    OperationId(String::from(id))
}

#[test]
fn repeated_bootstrap_applies_once() {
    let mut log = OperationLog::new(Vec::new());
    let id = op_id("boot-1");
    let op = AdminOp::Bootstrap {
        node_name: String::from("no-a"),
    };

    let first = log.apply(id.clone(), op.clone());
    let second = log.apply(id.clone(), op.clone());

    assert!(matches!(first, Ok(OpOutcome::Applied(_))));
    assert!(matches!(second, Ok(OpOutcome::AlreadyApplied(_))));
    assert!(log.reconcile(&id).is_some());
}

#[test]
fn same_id_with_distinct_payload_is_refused() {
    let mut log = log_with("dds-agent");
    let id = op_id("op-1");
    let running = AdminOp::SetService {
        service: String::from("dds-agent"),
        running: true,
    };
    let stopped = AdminOp::SetService {
        service: String::from("dds-agent"),
        running: false,
    };

    log.apply(id.clone(), running).expect("primeira aplica");
    let err = log
        .apply(id.clone(), stopped)
        .expect_err("payload distinto deve falhar");

    assert_eq!(err, NodeError::OperationIdConflict);
    let kept = log.reconcile(&id).expect("registro original preservado");
    assert_eq!(
        kept.op,
        AdminOp::SetService {
            service: String::from("dds-agent"),
            running: true,
        }
    );
}

#[test]
fn service_outside_own_scope_is_refused() {
    let mut log = log_with("dds-agent");

    let err = log
        .apply(
            op_id("op-9"),
            AdminOp::SetService {
                service: String::from("postgres-alheio"),
                running: true,
            },
        )
        .expect_err("servico alheio deve falhar");

    assert_eq!(err, NodeError::OutOfScope(String::from("postgres-alheio")));
    assert!(log.reconcile(&op_id("op-9")).is_none());
}

#[test]
fn reconcile_returns_none_for_unknown_id() {
    let log = log_with("dds-agent");

    assert!(log.reconcile(&op_id("inexistente")).is_none());
}

/// REQ/T-820-18: retenção limitada — acima de `MAX_RECORDS`, as
/// operações mais antigas são descartadas (FIFO) e as recentes ficam.
#[test]
fn log_retains_only_last_max_records() {
    let mut log = log_with("dds-agent");
    let total = 1050;
    for i in 0..total {
        log.apply(
            op_id(&format!("op-{i}")),
            AdminOp::SetService {
                service: String::from("dds-agent"),
                running: i % 2 == 0,
            },
        )
        .expect("aplica");
    }

    assert_eq!(log.records().count(), 1000);
    // Mais antiga descartada; mais recente presente.
    assert!(log.reconcile(&op_id("op-0")).is_none());
    assert!(log.reconcile(&op_id("op-49")).is_none());
    assert!(log.reconcile(&op_id("op-50")).is_some());
    assert!(log.reconcile(&op_id("op-1049")).is_some());
    // `wanted` continua lendo o último SetService remanescente.
    assert_eq!(log.wanted("dds-agent"), Some(false));
}

fn temp_db(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "studio-node-test-{name}-{}.json",
        std::process::id()
    ))
}

#[test]
fn save_and_load_round_trip_preserves_log() {
    let path = temp_db("roundtrip");
    let mut log = log_with("dds-agent");
    publish(&mut log, "op-1");

    log.save(&path).expect("salvar deve funcionar");
    let back = OperationLog::load(&path).expect("carregar deve funcionar");
    let _ = std::fs::remove_file(&path);

    assert!(back.reconcile(&op_id("op-1")).is_some());
    assert!(back.reconcile(&op_id("inexistente")).is_none());
}

#[test]
fn wanted_tracks_latest_set_service_in_order() {
    use crate::protocol::AdminOp;
    let mut log = log_with("s");
    for (id, running) in [("op-1", true), ("op-2", false)] {
        log.apply(
            op_id(id),
            AdminOp::SetService {
                service: String::from("s"),
                running,
            },
        )
        .expect("aplica");
    }

    assert_eq!(log.wanted("s"), Some(false));
    assert_eq!(log.wanted("outro"), None);
}

#[test]
fn load_missing_file_is_storage_error() {
    let path = temp_db("ausente");
    let _ = std::fs::remove_file(&path);

    let err = OperationLog::load(&path).expect_err("arquivo ausente deve falhar");

    assert!(matches!(err, NodeError::Storage(_)));
}

fn publish(log: &mut OperationLog, id: &str) {
    use crate::protocol::AdminOp;
    log.apply(
        op_id(id),
        AdminOp::SetService {
            service: String::from("dds-agent"),
            running: true,
        },
    )
    .expect("publicacao de teste deve aplicar");
}
