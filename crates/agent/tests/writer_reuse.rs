#![cfg(feature = "dds")]

use agent::engine::{Engine, EngineError, InferRequest, ProviderConstraint};
use agent::engine_dds::DdsEngine;
use cyclonedds::{DataReader, DomainParticipant, StatusExt, Subscriber, Topic};
use dds_contract::generated::orchestrator::LLMInferenceRequest;
use dds_contract::topics;
use dds_dataspace::qos::profiles;
use futures_util::StreamExt;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_persiste_entre_multiplas_inferencias() {
    let domain = 105;
    let participant = DomainParticipant::new(domain).expect("observer participant");
    let subscriber = Subscriber::new(&participant).expect("observer subscriber");
    let qos = profiles::llm().expect("LLM QoS");
    let topic =
        Topic::<LLMInferenceRequest>::with_qos(&participant, topics::LLM_REQUEST, Some(&qos))
            .expect("LLM request topic");
    let reader = DataReader::<LLMInferenceRequest>::with_qos(&subscriber, &topic, Some(&qos))
        .expect("observer reader");

    let engine = DdsEngine::new(domain, "agent-writer-reuse".into()).expect("DDS engine");
    assert_eq!(
        engine.provider_constraint(),
        ProviderConstraint::LocalOnly,
        "o engine DDS deve impedir cloud por padrão"
    );
    tokio::time::sleep(Duration::from_secs(1)).await;

    let before = reader
        .subscription_matched_status()
        .expect("subscription status before inference");

    let request = |id: &str| InferRequest {
        request_id: id.into(),
        messages_json: "[]".into(),
        model_name: "test".into(),
        temperature: 0.0,
        max_tokens: 1,
        stream: true,
        timeout_ms: 1,
    };
    for id in ["writer-reuse-1", "writer-reuse-2"] {
        let mut stream = engine.infer_stream(request(id));
        let result = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("inference stream must make progress")
            .expect("inference stream must yield its timeout");
        assert!(matches!(result, Err(EngineError::Timeout(1))));
    }

    let mut requests = Vec::new();
    for _ in 0..20 {
        requests.extend(reader.take_async().await.expect("take requests"));
        if requests.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let after = reader
        .subscription_matched_status()
        .expect("subscription status after inference");
    assert_eq!(before.total_count, 1, "writer deve existir no construtor");
    assert_eq!(
        after.total_count, 1,
        "inferências não podem criar writers adicionais"
    );
    assert_eq!(
        requests.len(),
        2,
        "ambas as inferências devem publicar requests"
    );
    assert!(requests
        .iter()
        .all(|request| request.provider_constraint == "LOCAL_ONLY"));
}
