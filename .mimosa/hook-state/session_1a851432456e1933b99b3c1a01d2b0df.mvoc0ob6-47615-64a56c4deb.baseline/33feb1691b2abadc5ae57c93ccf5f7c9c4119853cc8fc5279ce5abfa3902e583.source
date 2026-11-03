//! Round-trip DDS do Policy Engine em domínio isolado (92): dois participantes
//! reais — o serviço (publicador) e um consumidor — trocam
//! `Security.PolicySnapshot` e `Security.PolicyUpdate` pelo CycloneDDS.
//!
//! Ciclo validado (pub/sub completo, isolado por domínio):
//! 1. serviço lê o arquivo e publica o snapshot (TRANSIENT_LOCAL → o
//!    consumidor recebe mesmo tendo assinado depois);
//! 2. consumidor publica um `SecurityPolicyUpdate` (UPDATE_RULE);
//! 3. serviço aplica o delta e republica o snapshot com `new_version`;
//! 4. consumidor recebe o snapshot atualizado e a regra merged funciona.
//!
//! Rode com:
//! `CYCLONEDDS_STATIC=1 cargo test -p policy-engine --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, SecurityPolicyUpdate};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures::StreamExt;
use policy_engine::service::PolicyEngineService;

/// Domínio isolado do teste (>= 90, longe dos demais testes do workspace).
const DOMAIN: u32 = 92;

const POLICY_V1: &str = r#"{
    "version": 1,
    "rules": {
        "llm_inference": {
            "allowed_agents": ["AgenteA"],
            "agent_policies": {"AgenteA": {"allowed_security_levels": ["PUBLIC"]}},
            "default_action": "DENY"
        }
    }
}"#;

fn temp_policy_file(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "policy-engine-dds-test-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let mut f = std::fs::File::create(&path).expect("cria arquivo temporário");
    f.write_all(contents.as_bytes()).expect("escreve políticas");
    path
}

/// Espera um snapshot com `version >= min_version` (até ~15 s).
async fn wait_snapshot(
    snaps: &mut std::pin::Pin<Box<dyn futures::Stream<Item = SecurityPolicySnapshot> + Send>>,
    min_version: i32,
) -> SecurityPolicySnapshot {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(
            !remaining.is_zero(),
            "timeout esperando snapshot v{min_version}"
        );
        match tokio::time::timeout(remaining, snaps.next()).await {
            Ok(Some(snap)) if snap.version >= min_version => return snap,
            Ok(Some(_)) => {} // versão antiga — continua esperando
            Ok(None) => panic!("stream de snapshots encerrado"),
            Err(_) => panic!("timeout esperando snapshot v{min_version}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_update_roundtrip_dds() {
    let policy_path = temp_policy_file(POLICY_V1);
    let _cleanup = scopeguard_remove(policy_path.clone());

    // ── Participante A: o serviço Policy Engine ──────────────────────────
    let ds_service = Arc::new(
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR)
            .expect("DataSpace do serviço sobe"),
    );
    let service = PolicyEngineService::new(Arc::clone(&ds_service), policy_path)
        // Intervalo grande: no teste só interessam a carga inicial e os deltas.
        .with_republish_interval(Duration::from_secs(3600));
    let service_task = tokio::spawn(async move { service.run().await });

    // ── Participante B: consumidor ───────────────────────────────────────
    let ds_consumer = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR)
        .expect("DataSpace consumidor sobe");
    let mut snaps = ds_consumer.subscribe_security_snapshots();

    // 1. Snapshot inicial (TRANSIENT_LOCAL: late-join recebe a última amostra).
    let snap1 = wait_snapshot(&mut snaps, 1).await;
    assert_eq!(snap1.policy_id, "default");
    assert_eq!(snap1.version, 1);
    assert_eq!(snap1.published_by, "policy-engine-v1");
    assert!(snap1.policy_json.contains("AgenteA"));

    // 2. Consumidor publica um UPDATE_RULE. O tópico de updates é VOLATILE:
    // re-publicamos até o serviço estar com o reader pronto (delta idempotente).
    let update = SecurityPolicyUpdate {
        policy_id: "default".into(),
        previous_version: 1,
        new_version: 2,
        operation: "UPDATE_RULE".into(),
        rule_delta_json:
            r#"{"rules": {"llm_inference": {"allowed_agents": ["AgenteA", "AgenteViaDDS"]}}}"#
                .into(),
        published_by: "consumidor-teste".into(),
        timestamp_ns: 1,
    };
    let snap2 = loop {
        ds_consumer
            .write_security_update(update.clone())
            .await
            .expect("escreve update");
        match tokio::time::timeout(Duration::from_secs(2), wait_snapshot(&mut snaps, 2)).await {
            Ok(snap) => break snap,
            Err(_) => continue, // serviço ainda não estava escutando — reenvia
        }
    };

    // 3/4. Snapshot republicado com o delta aplicado.
    assert_eq!(snap2.version, 2);
    let doc =
        policy_engine::PolicyDocument::from_json_str(&snap2.policy_json).expect("json válido");
    assert_eq!(doc.version(), 2);
    assert_eq!(
        doc.check_llm_request("AgenteA", 0),
        policy_engine::PolicyDecision::Allowed,
        "regra original preservada após o merge"
    );
    assert!(
        matches!(
            doc.check_llm_request("AgenteViaDDS", 0),
            policy_engine::PolicyDecision::Denied(_)
        ),
        "agente adicionado via DDS entra em allowed_agents, mas sem agent_policy → DENY (default_action)"
    );

    service_task.abort();
}

/// Remove o arquivo temporário ao final do teste (sem dep extra).
fn scopeguard_remove(path: PathBuf) -> impl Drop {
    struct Guard(PathBuf);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    Guard(path)
}
