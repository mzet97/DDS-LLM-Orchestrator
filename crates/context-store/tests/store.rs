//! Testes do `LocalContextStore` (sem DDS): put/get, merge de deltas, TTL e
//! durabilidade do journal JSONL — paridade com `postgres_store.py`.

use context_store::{
    ContextStore, LocalContextStore, StoreError, DEFAULT_TTL_SECONDS, UPDATE_APPEND, UPDATE_CLEAR,
    UPDATE_REPLACE,
};
use dds_contract::generated::dds_llm_orchestrator::{ContextSnapshot, ContextUpdate};
use std::sync::atomic::{AtomicU64, Ordering};

fn snap(
    context_id: &str,
    session_id: &str,
    messages_json: &str,
    ttl_seconds: u32,
) -> ContextSnapshot {
    ContextSnapshot {
        context_id: context_id.to_string(),
        client_id: "cliente-1".to_string(),
        session_id: session_id.to_string(),
        messages_json: messages_json.to_string(),
        metadata_json: r#"{"origem":"teste"}"#.to_string(),
        security_level: 1,
        created_at_ns: 1_000,
        updated_at_ns: 2_000,
        ttl_seconds,
    }
}

fn update(context_id: &str, update_type: i32, delta: &str) -> ContextUpdate {
    ContextUpdate {
        context_id: context_id.to_string(),
        update_type,
        messages_delta_json: delta.to_string(),
        metadata_delta_json: "{}".to_string(),
        updated_at_ns: 3_000,
    }
}

async fn messages_async(store: &LocalContextStore, context_id: &str) -> serde_json::Value {
    let s = store
        .get(context_id)
        .await
        .unwrap()
        .expect("contexto existe");
    serde_json::from_str(&s.messages_json).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_e_get() {
    let store = LocalContextStore::in_memory();
    store
        .put_snapshot(&snap(
            "ctx-1",
            "sess-1",
            r#"[{"role":"user","content":"oi"}]"#,
            3600,
        ))
        .await
        .unwrap();

    let got = store.get("ctx-1").await.unwrap().expect("ctx-1 existe");
    assert_eq!(got.client_id, "cliente-1");
    assert_eq!(got.session_id, "sess-1");
    assert_eq!(got.security_level, 1);
    assert_eq!(got.created_at_ns, 1_000);
    assert_eq!(got.updated_at_ns, 2_000);
    assert_eq!(got.ttl_seconds, 3600);

    assert!(store.get("inexistente").await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_upsert_preserva_identidade_do_primeiro_insert() {
    let store = LocalContextStore::in_memory();
    store
        .put_snapshot(&snap("ctx-1", "sess-1", r#"[{"n":1}]"#, 3600))
        .await
        .unwrap();

    // Re-put com identidade diferente: ON CONFLICT atualiza só os mutáveis.
    let mut segundo = snap("ctx-1", "sess-OUTRA", r#"[{"n":2}]"#, 60);
    segundo.client_id = "cliente-2".to_string();
    segundo.created_at_ns = 9_999;
    segundo.updated_at_ns = 8_888;
    segundo.security_level = 3;
    segundo.metadata_json = r#"{"novo":true}"#.to_string();
    store.put_snapshot(&segundo).await.unwrap();

    let got = store.get("ctx-1").await.unwrap().unwrap();
    // mutáveis atualizados
    assert_eq!(got.messages_json, r#"[{"n":2}]"#);
    assert_eq!(got.metadata_json, r#"{"novo":true}"#);
    assert_eq!(got.security_level, 3);
    assert_eq!(got.updated_at_ns, 8_888);
    assert_eq!(got.ttl_seconds, 60);
    // identidade preservada (espelha o ON CONFLICT DO UPDATE do PostgreSQL)
    assert_eq!(got.client_id, "cliente-1");
    assert_eq!(got.session_id, "sess-1");
    assert_eq!(got.created_at_ns, 1_000);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_update_append_cria_contexto_com_defaults() {
    let store = LocalContextStore::in_memory();
    store
        .apply_update(&update(
            "ctx-novo",
            UPDATE_APPEND,
            r#"[{"role":"user","content":"oi"}]"#,
        ))
        .await
        .unwrap();

    let got = store
        .get("ctx-novo")
        .await
        .unwrap()
        .expect("criado pelo update");
    // defaults de models.py
    assert_eq!(got.client_id, "");
    assert_eq!(got.session_id, "");
    assert_eq!(got.metadata_json, "{}");
    assert_eq!(got.security_level, 0);
    assert_eq!(got.ttl_seconds, DEFAULT_TTL_SECONDS);
    assert_eq!(got.updated_at_ns, 3_000); // bumped pelo update
    assert!(got.created_at_ns > 0);
    let v: serde_json::Value = serde_json::from_str(&got.messages_json).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_update_append_faz_merge_do_historico() {
    let store = LocalContextStore::in_memory();
    store
        .put_snapshot(&snap(
            "ctx-1",
            "sess-1",
            r#"[{"role":"user","content":"a"}]"#,
            3600,
        ))
        .await
        .unwrap();
    store
        .apply_update(&update(
            "ctx-1",
            UPDATE_APPEND,
            r#"[{"role":"assistant","content":"b"}]"#,
        ))
        .await
        .unwrap();

    let v = messages_async(&store, "ctx-1").await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["content"], "a");
    assert_eq!(arr[1]["content"], "b");
    // updated_at bumped pelo update
    assert_eq!(
        store.get("ctx-1").await.unwrap().unwrap().updated_at_ns,
        3_000
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_update_replace_e_clear() {
    let store = LocalContextStore::in_memory();
    store
        .put_snapshot(&snap("ctx-1", "sess-1", r#"[{"old":true}]"#, 3600))
        .await
        .unwrap();

    store
        .apply_update(&update(
            "ctx-1",
            UPDATE_REPLACE,
            r#"[{"role":"system","content":"novo"}]"#,
        ))
        .await
        .unwrap();
    let got = store.get("ctx-1").await.unwrap().unwrap();
    assert_eq!(got.messages_json, r#"[{"role":"system","content":"novo"}]"#); // verbatim

    store
        .apply_update(&update("ctx-1", UPDATE_CLEAR, "[]"))
        .await
        .unwrap();
    assert_eq!(
        store.get("ctx-1").await.unwrap().unwrap().messages_json,
        "[]"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_update_tipo_desconhecido_preserva_mensagens_mas_faz_bump() {
    let store = LocalContextStore::in_memory();
    store
        .put_snapshot(&snap("ctx-1", "sess-1", r#"[{"old":true}]"#, 3600))
        .await
        .unwrap();
    store
        .apply_update(&update("ctx-1", 99, r#"[{"x":1}]"#))
        .await
        .unwrap();

    let got = store.get("ctx-1").await.unwrap().unwrap();
    assert_eq!(got.messages_json, r#"[{"old":true}]"#);
    assert_eq!(got.updated_at_ns, 3_000);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_update_delta_nao_array_falha_sem_persistir() {
    let store = LocalContextStore::in_memory();
    let err = store
        .apply_update(&update("ctx-err", UPDATE_APPEND, r#"{"nao":"array"}"#))
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::NotJsonArray { .. }));
    // como no Python (exceção antes do save): nada foi persistido
    assert!(store.get("ctx-err").await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expire_ttl_remove_vencidos() {
    let store = LocalContextStore::in_memory();
    store
        .put_snapshot(&snap("ctx-vivo", "sess-1", "[]", 3600))
        .await
        .unwrap();
    store
        .put_snapshot(&snap("ctx-morto", "sess-1", "[]", 0)) // ttl=0 → já nasce vencido
        .await
        .unwrap();

    let removed = store.expire_ttl().await.unwrap();
    assert_eq!(removed, 1);
    assert!(store.get("ctx-morto").await.unwrap().is_none());
    assert!(store.get("ctx-vivo").await.unwrap().is_some());

    // segunda varredura: nada a remover
    assert_eq!(store.expire_ttl().await.unwrap(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_sessions_distintas_ordenadas_sem_vazias() {
    let store = LocalContextStore::in_memory();
    store
        .put_snapshot(&snap("c1", "sess-b", "[]", 3600))
        .await
        .unwrap();
    store
        .put_snapshot(&snap("c2", "sess-a", "[]", 3600))
        .await
        .unwrap();
    store
        .put_snapshot(&snap("c3", "sess-a", "[]", 3600))
        .await
        .unwrap(); // duplicada
    store
        .put_snapshot(&snap("c4", "", "[]", 3600))
        .await
        .unwrap(); // sem sessão

    let sessions = store.list_sessions().await.unwrap();
    assert_eq!(sessions, vec!["sess-a".to_string(), "sess-b".to_string()]);
}

// ── Journal JSONL (durabilidade leve) ──────────────────────────────────────

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_journal_path() -> std::path::PathBuf {
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "context-store-test-{}-{nanos}-{seq}.jsonl",
        std::process::id()
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn journal_recupera_estado_apos_reabrir() {
    let path = temp_journal_path();

    {
        let store = LocalContextStore::open(&path).await.unwrap();
        store
            .put_snapshot(&snap(
                "ctx-1",
                "sess-1",
                r#"[{"role":"user","content":"a"}]"#,
                3600,
            ))
            .await
            .unwrap();
        store
            .apply_update(&update(
                "ctx-1",
                UPDATE_APPEND,
                r#"[{"role":"assistant","content":"b"}]"#,
            ))
            .await
            .unwrap();
        store
            .apply_update(&update(
                "ctx-2",
                UPDATE_APPEND,
                r#"[{"role":"user","content":"solo"}]"#,
            ))
            .await
            .unwrap();
        assert_eq!(store.journaled_ops(), 3);
    } // drop = "restart"

    {
        let store = LocalContextStore::open(&path).await.unwrap();
        assert_eq!(store.len(), 2);
        let v = messages_async(&store, "ctx-1").await;
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2, "merge do update sobreviveu ao replay");
        assert_eq!(arr[1]["content"], "b");
        // identidade do put original preservada apesar do update posterior
        let got = store.get("ctx-1").await.unwrap().unwrap();
        assert_eq!(got.client_id, "cliente-1");
        assert_eq!(got.session_id, "sess-1");
        assert_eq!(got.created_at_ns, 1_000);
        assert_eq!(got.updated_at_ns, 3_000);
        assert_eq!(store.journaled_ops(), 3, "replay contou as ops");
    }

    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn journal_linha_corrompida_e_ignorada_no_replay() {
    let path = temp_journal_path();

    {
        let store = LocalContextStore::open(&path).await.unwrap();
        store
            .put_snapshot(&snap("ctx-1", "sess-1", r#"[{"ok":true}]"#, 3600))
            .await
            .unwrap();
    }
    // injeta uma linha inválida no meio do journal
    let mut content = std::fs::read_to_string(&path).unwrap();
    content.push_str("{linha corrompida\n");
    std::fs::write(&path, content).unwrap();

    {
        let store = LocalContextStore::open(&path).await.unwrap();
        let got = store.get("ctx-1").await.unwrap();
        assert!(
            got.is_some(),
            "registro válido sobreviveu à linha corrompida"
        );
    }

    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn journal_preserva_expiracao_absoluta_no_replay() {
    let path = temp_journal_path();
    {
        let store = LocalContextStore::open(&path).await.unwrap();
        store
            .put_snapshot(&snap("ctx-morto", "s", "[]", 0))
            .await
            .unwrap();
        store
            .put_snapshot(&snap("ctx-vivo", "s", "[]", 3600))
            .await
            .unwrap();
    }
    {
        // após "restart", o contexto vencido continua vencido (expires_at absoluto)
        let store = LocalContextStore::open(&path).await.unwrap();
        let removed = store.expire_ttl().await.unwrap();
        assert_eq!(removed, 1);
        assert!(store.get("ctx-morto").await.unwrap().is_none());
        assert!(store.get("ctx-vivo").await.unwrap().is_some());
    }
    let _ = std::fs::remove_file(&path);
}
