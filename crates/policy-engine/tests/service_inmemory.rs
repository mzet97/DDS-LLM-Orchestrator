//! Teste do serviço Policy Engine contra o `InMemoryDataSpace` (sem DDS):
//! detecção de mudança, publicação de snapshot, cache com TTL e aplicação
//! de deltas (`SecurityPolicyUpdate`) com republicação.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::SecurityPolicyUpdate;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::in_memory::InMemoryDataSpace;
use futures::StreamExt;
use policy_engine::service::PolicyEngineService;

/// Arquivo de políticas temporário (limpo no drop).
struct TempPolicyFile(PathBuf);

impl TempPolicyFile {
    fn new(contents: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "policy-engine-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut f = std::fs::File::create(&path).expect("cria arquivo temporário");
        f.write_all(contents.as_bytes()).expect("escreve políticas");
        Self(path)
    }

    fn rewrite(&self, contents: &str) {
        std::fs::write(&self.0, contents).expect("reescreve políticas");
    }
}

impl Drop for TempPolicyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publica_snapshot_na_mudanca_de_versao() {
    let file = TempPolicyFile::new(POLICY_V1);
    let ds = Arc::new(InMemoryDataSpace::new());
    let service = PolicyEngineService::new(Arc::clone(&ds), file.0.clone());

    // Consumidor assina ANTES da publicação.
    let mut snaps = ds.subscribe_security_snapshots();

    assert!(service.load_and_publish().await.expect("carga inicial"));
    let snap = snaps.next().await.expect("snapshot recebido");
    assert_eq!(snap.policy_id, "default");
    assert_eq!(snap.version, 1);
    assert_eq!(snap.published_by, "policy-engine-v1");
    assert!(snap.policy_json.contains("AgenteA"));

    // Cache local alimentado com o documento.
    assert!(service.cache().get("default").is_some());
    assert_eq!(service.current_state("default").map(|(v, _)| v), Some(1));

    // Mesma versão E mesmo conteúdo → não republica (fiel ao Python).
    assert!(!service.load_and_publish().await.expect("sem mudança"));

    // Conteúdo mudou (mesma versão) → republica.
    file.rewrite(&POLICY_V1.replace("AgenteA", "AgenteB"));
    assert!(service.load_and_publish().await.expect("mudou conteúdo"));
    let snap2 = snaps.next().await.expect("2º snapshot");
    assert!(snap2.policy_json.contains("AgenteB"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn arquivo_invalido_mantem_versao_atual() {
    let file = TempPolicyFile::new("isso não é JSON");
    let ds = Arc::new(InMemoryDataSpace::new());
    let service = PolicyEngineService::new(ds, file.0.clone());

    // Falha de parse: só loga e retorna Ok(false) — o serviço não cai.
    assert!(!service
        .load_and_publish()
        .await
        .expect("não propaga erro de parse"));
    assert!(service.current_state("default").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aplica_delta_e_republica() {
    let file = TempPolicyFile::new(POLICY_V1);
    let ds = Arc::new(InMemoryDataSpace::new());
    let service = PolicyEngineService::new(Arc::clone(&ds), file.0.clone());
    assert!(service.load_and_publish().await.expect("carga inicial"));

    let mut snaps = ds.subscribe_security_snapshots();
    let update = SecurityPolicyUpdate {
        policy_id: "default".into(),
        previous_version: 1,
        new_version: 2,
        operation: "UPDATE_RULE".into(),
        rule_delta_json:
            r#"{"rules": {"llm_inference": {"allowed_agents": ["AgenteA", "AgenteC"]}}}"#.into(),
        published_by: "teste".into(),
        timestamp_ns: 1,
    };
    assert!(service.handle_update(&update).await.expect("aplica delta"));

    let snap = snaps.next().await.expect("snapshot republicado");
    assert_eq!(snap.version, 2);
    let doc = policy_engine::PolicyDocument::from_json_str(&snap.policy_json).expect("json");
    assert_eq!(
        doc.version(),
        2,
        "version do documento acompanha new_version"
    );
    assert!(matches!(
        doc.check_llm_request("AgenteC", 0),
        policy_engine::PolicyDecision::Denied(_)
    ));
    // AgenteC autorizado como agente, mas sem entrada em agent_policies → DENY
    // no nível (fiel ao default_action=DENY do Python). Já AgenteA segue OK:
    assert_eq!(
        doc.check_llm_request("AgenteA", 0),
        policy_engine::PolicyDecision::Allowed
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delta_sobre_policy_ausente_usa_documento_vazio() {
    let file = TempPolicyFile::new(POLICY_V1);
    let ds = Arc::new(InMemoryDataSpace::new());
    let service = PolicyEngineService::new(ds, file.0.clone());

    let update = SecurityPolicyUpdate {
        policy_id: "nova-policy".into(),
        previous_version: 0,
        new_version: 1,
        operation: "ADD_RULE".into(),
        rule_delta_json: r#"{"rules": {"tool_call": {"high_risk_tools": ["rm"]}}}"#.into(),
        published_by: "teste".into(),
        timestamp_ns: 1,
    };
    assert!(service.handle_update(&update).await.expect("aplica"));
    let (v, _doc) = service.current_state("nova-policy").expect("estado criado");
    assert_eq!(v, 1);
}

#[test]
fn republish_interval_default_e_60s() {
    // Fiel ao Python (`await asyncio.wait_for(self._stop.wait(), timeout=60.0)`).
    assert_eq!(
        policy_engine::service::DEFAULT_REPUBLISH_INTERVAL,
        Duration::from_secs(60)
    );
}
