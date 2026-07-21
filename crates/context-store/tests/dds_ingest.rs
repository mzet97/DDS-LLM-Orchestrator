//! Teste DDS do Context Store (ciclo completo): um participante publica
//! `Context.Snapshot` + `Context.Update` e o serviço (outro participante, no
//! mesmo domínio isolado) persiste no store — snapshot depois merge do delta.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p context-store --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use context_store::{ContextStore, ContextStoreService, LocalContextStore};
use dds_contract::generated::dds_llm_orchestrator::{ContextSnapshot, ContextUpdate};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Domínio isolado do teste (>= 91, fora da faixa usada pelas outras crates).
const DOMAIN: u32 = 91;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn snapshot(context_id: &str) -> ContextSnapshot {
    ContextSnapshot {
        context_id: context_id.to_string(),
        client_id: "cliente-dds".to_string(),
        session_id: "sess-dds".to_string(),
        messages_json: r#"[{"role":"user","content":"pergunta"}]"#.to_string(),
        metadata_json: "{}".to_string(),
        security_level: 0,
        created_at_ns: now_ns(),
        updated_at_ns: now_ns(),
        ttl_seconds: 3600,
    }
}

fn append_update(context_id: &str) -> ContextUpdate {
    ContextUpdate {
        context_id: context_id.to_string(),
        update_type: 0, // APPEND
        messages_delta_json: r#"[{"role":"assistant","content":"resposta"}]"#.to_string(),
        metadata_delta_json: "{}".to_string(),
        updated_at_ns: now_ns(),
    }
}

/// Faz poll no store até `cond` valer ou estourar o orçamento.
async fn wait_until<F>(what: &str, budget: Duration, mut cond: F) -> bool
where
    F: FnMut() -> bool,
{
    let t0 = Instant::now();
    while t0.elapsed() < budget {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("[dds_ingest] timeout esperando: {what}");
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_e_update_chegam_ao_store() {
    // Participante 1 (subscriber): o serviço Context Store.
    let store = Arc::new(LocalContextStore::in_memory());
    let service = ContextStoreService::new(DOMAIN, Arc::clone(&store)).expect("serviço sobe");

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let svc = tokio::spawn(async move {
        service
            .run(async move {
                let _ = stop_rx.await;
            })
            .await;
    });

    // Participante 2 (publisher): publica snapshot e update no mesmo domínio.
    let ds_pub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("ds_pub sobe");

    // Settle: SEDP/match entre os dois participantes (padrão dos testes do workspace).
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // 1) Snapshot chega ao store.
    ds_pub
        .write_context_snapshot(snapshot("ctx-dds"))
        .await
        .expect("escreve snapshot");
    let store_ref = Arc::clone(&store);
    let got_snap = wait_until("snapshot no store", Duration::from_secs(10), move || {
        store_ref.contains("ctx-dds")
    })
    .await;
    assert!(got_snap, "snapshot não chegou ao store");
    let got = store.get("ctx-dds").await.unwrap().expect("ctx-dds existe");
    assert_eq!(got.session_id, "sess-dds");
    let v: serde_json::Value = serde_json::from_str(&got.messages_json).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);

    // 2) Update (APPEND) faz o merge no mesmo contexto.
    ds_pub
        .write_context_update(append_update("ctx-dds"))
        .await
        .expect("escreve update");
    let store_ref = Arc::clone(&store);
    let merged = wait_until("merge do update", Duration::from_secs(10), move || {
        store_ref.messages_len("ctx-dds") == Some(2)
    })
    .await;
    assert!(merged, "update não foi aplicado ao store");
    let v = {
        let got = store.get("ctx-dds").await.unwrap().unwrap();
        serde_json::from_str::<serde_json::Value>(&got.messages_json).unwrap()
    };
    let arr = v.as_array().unwrap();
    assert_eq!(arr[0]["content"], "pergunta");
    assert_eq!(arr[1]["content"], "resposta");

    // Encerra: serviço primeiro (para de consumir), depois o publisher.
    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(5), svc)
        .await
        .expect("serviço encerra")
        .expect("task do serviço ok");
    ds_pub.shutdown().await.expect("ds_pub encerra");
}
