//! DdsEngine — ponte com o llama-server C++ via tópicos `LLM.*` (REQ-204, T-205).
//!
//! Publica `LLMInferenceRequest` e consome `LLMInferenceResult`/`LLMInferenceError`
//! correlacionados por `request_id`. Cada chamada cria readers dedicados
//! ('static, sem corrida de take entre slots concorrentes).

use crate::engine::{Chunk, Engine, EngineError, InferRequest, ProviderConstraint};
use async_stream::stream;
use cyclonedds::{DataReader, DataWriter, DomainParticipant, Publisher, Subscriber, Topic};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult,
};
use dds_contract::topics;
use dds_dataspace::qos::profiles;
use futures_core::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Engine DDS real: llama-server (ou gateway compatível com `orchestrator::LLM*`).
/// Writer é reusado entre chamadas para evitar leak de entidades DDS.
/// Readers são criados por stream (take independente entre slots concorrentes).
pub struct DdsEngine {
    #[allow(dead_code)]
    participant: DomainParticipant,
    subscriber: Arc<Subscriber>,
    request_writer: Arc<DataWriter<LLMInferenceRequest>>,
    res_topic: Arc<Topic<LLMInferenceResult>>,
    err_topic: Arc<Topic<LLMInferenceError>>,
    agent_id: String,
    provider_constraint: ProviderConstraint,
}

impl DdsEngine {
    pub fn new(domain_id: u32, agent_id: String) -> Result<Self, EngineError> {
        Self::new_with_constraint(domain_id, agent_id, ProviderConstraint::default())
    }

    pub fn new_with_constraint(
        domain_id: u32,
        agent_id: String,
        provider_constraint: ProviderConstraint,
    ) -> Result<Self, EngineError> {
        let err = |e: cyclonedds::DdsError| EngineError::DdsError(e.to_string());
        let participant = DomainParticipant::new(domain_id).map_err(err)?;
        let publisher = Publisher::new(&participant).map_err(err)?;
        let subscriber = Subscriber::new(&participant).map_err(err)?;

        let qos = profiles::llm().map_err(err)?;
        let qos_result = profiles::llm_result().map_err(err)?;
        let req_topic =
            Topic::<LLMInferenceRequest>::with_qos(&participant, topics::LLM_REQUEST, Some(&qos))
                .map_err(err)?;
        let res_topic = Topic::<LLMInferenceResult>::with_qos(
            &participant,
            topics::LLM_RESULT,
            Some(&qos_result),
        )
        .map_err(err)?;
        let err_topic =
            Topic::<LLMInferenceError>::with_qos(&participant, topics::LLM_ERROR, Some(&qos))
                .map_err(err)?;
        let request_writer =
            DataWriter::with_qos(&publisher, &req_topic, Some(&qos)).map_err(err)?;

        Ok(Self {
            participant,
            subscriber: Arc::new(subscriber),
            request_writer: Arc::new(request_writer),
            res_topic: Arc::new(res_topic),
            err_topic: Arc::new(err_topic),
            agent_id,
            provider_constraint,
        })
    }

    pub const fn provider_constraint(&self) -> ProviderConstraint {
        self.provider_constraint
    }
}

impl Engine for DdsEngine {
    fn infer_stream(
        &self,
        req: InferRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<Chunk, EngineError>> + Send>> {
        let subscriber = Arc::clone(&self.subscriber);
        let req_writer = Arc::clone(&self.request_writer);
        let res_topic = Arc::clone(&self.res_topic);
        let err_topic = Arc::clone(&self.err_topic);
        let agent_id = self.agent_id.clone();
        let provider_constraint = self.provider_constraint;

        Box::pin(stream! {
            use futures_util::StreamExt;

            let qos = match profiles::llm() {
                Ok(q) => q,
                Err(e) => {
                    yield Err(EngineError::DdsError(e.to_string()));
                    return;
                }
            };
            // DDS-QOS-004 / Gate C2: o reader de Result usa o perfil profundo
            // (KeepLast 256) — com o perfil genérico (10) o RHC sobrescrevia
            // chunks antes do take drenar (perda medida: 108/128).
            let qos_result = match profiles::llm_result() {
                Ok(q) => q,
                Err(e) => {
                    yield Err(EngineError::DdsError(e.to_string()));
                    return;
                }
            };
            let res_reader = match DataReader::<LLMInferenceResult>::with_qos(&subscriber, &res_topic, Some(&qos_result)) {
                Ok(r) => r,
                Err(e) => {
                    yield Err(EngineError::DdsError(e.to_string()));
                    return;
                }
            };
            let err_reader = match DataReader::<LLMInferenceError>::with_qos(&subscriber, &err_topic, Some(&qos)) {
                Ok(r) => r,
                Err(e) => {
                    yield Err(EngineError::DdsError(e.to_string()));
                    return;
                }
            };

            // Settle curto anti-corrida de discovery (primeira request do processo).
            tokio::time::sleep(Duration::from_millis(250)).await;

            let llm_req = LLMInferenceRequest {
                request_id: req.request_id.clone(),
                task_id: req.request_id.clone(),
                agent_id,
                model_name: req.model_name.clone(),
                messages_json: req.messages_json.clone(),
                temperature: req.temperature,
                max_tokens: req.max_tokens,
                stream: true,
                security_level: 0,
                provider_constraint: provider_constraint.as_idl_literal().into(),
                created_at_ns: now_ns(),
            };

            if let Err(e) = req_writer.write(&llm_req) {
                yield Err(EngineError::DdsError(e.to_string()));
                return;
            }

            let deadline = Instant::now() + Duration::from_millis(req.timeout_ms);
            let req_id = req.request_id.clone();
            let mut res_stream = Box::pin(res_reader.take_aiter_timeout(200_000_000));
            let mut err_stream = Box::pin(err_reader.take_aiter_timeout(200_000_000));

            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    yield Err(EngineError::Timeout(req.timeout_ms));
                    return;
                }

                tokio::select! {
                    batch = res_stream.next() => {
                        match batch {
                            Some(Ok(results)) => {
                                for r in results {
                                    if r.request_id != req_id { continue; }
                                    let is_final = r.is_final;
                                    yield Ok(Chunk {
                                        content: r.content,
                                        seq_num: r.seq_num,
                                        is_final,
                                        tokens_prompt: r.tokens_prompt,
                                        tokens_completion: r.tokens_completion,
                                    });
                                    if is_final { return; }
                                }
                            }
                            Some(Err(e)) => {
                                yield Err(EngineError::DdsError(e.to_string()));
                                return;
                            }
                            None => return,
                        }
                    }
                    batch = err_stream.next() => {
                        match batch {
                            Some(Ok(errors)) => {
                                for e in errors {
                                    if e.request_id != req_id { continue; }
                                    yield Err(EngineError::InferenceFailed(format!(
                                        "{} ({}): {}", e.provider, e.error_code, e.error_message
                                    )));
                                    return;
                                }
                            }
                            Some(Err(e)) => {
                                yield Err(EngineError::DdsError(e.to_string()));
                                return;
                            }
                            None => {}
                        }
                    }
                    _ = tokio::time::sleep(remaining) => {
                        yield Err(EngineError::Timeout(req.timeout_ms));
                        return;
                    }
                }
            }
        })
    }
}
