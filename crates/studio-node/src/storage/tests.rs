use super::*;
use crate::operations::OpOutcome;
use crate::protocol::AdminOp;

fn op_set(service: &str, running: bool) -> AdminOp {
    AdminOp::SetService {
        service: String::from(service),
        running,
    }
}

fn seeded() -> OperationLog {
    let mut log = OperationLog::new(vec![String::from("dds-agent")]);
    log.apply(OperationId(String::from("op-1")), op_set("dds-agent", true))
        .expect("aplica op-1");
    log.apply(
        OperationId(String::from("op-2")),
        op_set("dds-agent", false),
    )
    .expect("aplica op-2");
    log
}

fn tempdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("studio-node-storage-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir temp");
    dir
}

#[test]
fn sqlite_roundtrip_preserves_state_and_order() {
    let dir = tempdir("roundtrip");
    let path = dir.join("operations.db");
    let log = seeded();
    {
        let storage = SqliteStorage::new(path.clone());
        storage.save(&log).expect("salva");
    }
    let storage = SqliteStorage::new(path);
    let reloaded = storage.load().expect("carrega").expect("estado");
    assert_eq!(reloaded.owned_services(), log.owned_services());
    assert_eq!(reloaded.wanted("dds-agent"), Some(false));
    // ordem preservada: op-1 é a mais antiga, op-2 a mais recente
    let order: Vec<_> = reloaded.order().iter().map(|id| id.0.clone()).collect();
    assert_eq!(order, vec![String::from("op-1"), String::from("op-2")]);
    // idempotência sobrevive ao restart: mesma op aplicada de novo → AlreadyApplied
    let mut reloaded = reloaded;
    assert!(matches!(
        reloaded.apply(
            OperationId(String::from("op-2")),
            op_set("dds-agent", false)
        ),
        Ok(OpOutcome::AlreadyApplied(_))
    ));
}

#[test]
fn sqlite_retention_window_survives_reload() {
    let dir = tempdir("retention");
    let path = dir.join("operations.db");
    let mut log = OperationLog::new(vec![String::from("dds-agent")]);
    for i in 0..1050 {
        log.apply(
            OperationId(format!("op-{i}")),
            op_set("dds-agent", i % 2 == 0),
        )
        .expect("aplica");
    }
    let storage = SqliteStorage::new(path);
    storage.save(&log).expect("salva");
    let reloaded = storage.load().expect("carrega").expect("estado");
    assert_eq!(reloaded.order().len(), log.order().len());
    assert_eq!(reloaded.records().count(), log.records().count());
}

/// G-34: restart adota o JSON sem migração destrutiva — original vira
/// `.imported` com conteúdo preservado; a 2ª carga vem do banco.
#[test]
fn migration_from_json_is_non_destructive_and_idempotent() {
    let dir = tempdir("migration");
    let db_path = dir.join("operations.db");
    let json_path = dir.join("operations.json");
    let log = seeded();
    let json_storage = JsonStorage::new(json_path.clone());
    json_storage.save(&log).expect("salva json");

    let sqlite = SqliteStorage::new(db_path);
    let reloaded = sqlite
        .load()
        .expect("carrega com migração")
        .expect("estado");
    assert_eq!(reloaded.wanted("dds-agent"), Some(false));
    let imported_path = dir.join("operations.json.imported");
    assert!(imported_path.exists(), "original renomeado, não apagado");
    assert!(
        !json_path.exists(),
        "json não pode permanecer no caminho ativo após migração"
    );
    let preserved = std::fs::read_to_string(&imported_path).expect("lê original");
    assert!(preserved.contains("dds-agent"), "conteúdo preservado");

    // migração idempotente: 2ª carga não toca nada (banco não está mais virgem)
    let again = sqlite.load().expect("2ª carga").expect("estado");
    assert_eq!(again.wanted("dds-agent"), Some(false));
    let still = std::fs::read_to_string(&imported_path).expect("original intacto");
    assert_eq!(preserved, still);
}

/// G-36: importação NÃO executa operações — o estado carregado reflete
/// o que estava no JSON (mesmo wanted), e nenhum efeito novo aparece
/// (nenhum registro adicional no log migrado).
#[test]
fn migration_imports_state_without_executing() {
    let dir = tempdir("no-exec");
    let db_path = dir.join("operations.db");
    let json_path = dir.join("operations.json");
    let mut log = OperationLog::new(vec![String::from("dds-agent")]);
    log.apply(
        OperationId(String::from("boot-1")),
        AdminOp::Bootstrap {
            node_name: String::from("no-a"),
        },
    )
    .expect("boot");
    log.apply(OperationId(String::from("op-1")), op_set("dds-agent", true))
        .expect("op-1");
    JsonStorage::new(json_path).save(&log).expect("salva");

    let sqlite = SqliteStorage::new(db_path);
    let mut reloaded = sqlite.load().expect("carrega").expect("estado");
    assert_eq!(reloaded.records().count(), log.records().count());
    assert_eq!(reloaded.wanted("dds-agent"), Some(true));
    assert!(reloaded
        .reconcile(&OperationId(String::from("boot-1")))
        .is_some());
    // payload distinto no id importado continua RECUSADO (histórico é história)
    let err = reloaded
        .apply(
            OperationId(String::from("op-1")),
            op_set("dds-agent", false),
        )
        .expect_err("conflito preservado");
    assert!(matches!(err, NodeError::OperationIdConflict));
}

#[test]
fn sqlite_corrupted_db_fails_fast() {
    let dir = tempdir("corrupt");
    let path = dir.join("operations.db");
    std::fs::write(&path, b"nao-e-um-banco-sqlite").expect("escreve lixo");
    let storage = SqliteStorage::new(path);
    assert!(storage.load().is_err(), "corrompido falha, não mascara");
}

#[test]
fn json_storage_roundtrip() {
    let dir = tempdir("json");
    let path = dir.join("operations.json");
    let log = seeded();
    let storage = JsonStorage::new(path);
    storage.save(&log).expect("salva");
    let reloaded = storage.load().expect("carrega").expect("estado");
    assert_eq!(reloaded.wanted("dds-agent"), Some(false));
    let absent = JsonStorage::new(dir.join("inexistente.json"));
    assert!(absent.load().expect("sem arquivo").is_none());
}
