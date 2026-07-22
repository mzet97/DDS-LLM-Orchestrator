//! DdsEngine — ponte com o llama-server C++ via tópicos `LLM.*` (REQ-204, T-205).
//!
//! Publica `LLMInferenceRequest` e consome `LLMInferenceResult`/`LLMInferenceError`
//! correlacionados por `request_id`. Cada chamada cria readers dedicados
//! ('static, sem corrida de take entre slots concorrentes).

use crate::engine::{Chunk, Engine, EngineError, InferRequest};
use async_stream::stream;
use cyclonedds::{
    DataReader, DataWriter, DdsEntity, DomainParticipant, Publisher, Subscriber, Topic,
};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult,
};
use dds_contract::topics;
use dds_dataspace::qos::profiles;
use futures_core::Stream;
use std::pin::Pin;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Engine DDS real: llama-server (ou gateway compatível com `orchestrator::LLM*`).
pub struct DdsEngine {
    #[allow(dead_code)]
    participant: DomainParticipant,
    publisher: Publisher,
    subscriber: Subscriber,
    req_topic: Topic<LLMInferenceRequest>,
    res_topic: Topic<LLMInferenceResult>,
    err_topic: Topic<LLMInferenceError>,
    agent_id: String,
}

impl DdsEngine {
    pub fn new(domain_id: u32, agent_id: String) -> Result<Self, EngineError> {
        let err = |e: cyclonedds::DdsError| EngineError::DdsError(e.to_string());
        let participant = DomainParticipant::new(domain_id).map_err(err)?;
        let publisher = Publisher::new(participant.entity()).map_err(err)?;
        let subscriber = Subscriber::new(participant.entity()).map_err(err)?;

        let qos = profiles::llm().map_err(err)?;
        let req_topic = Topic::<LLMInferenceRequest>::with_qos(
            participant.entity(),
            topics::LLM_REQUEST,
            Some(&qos),
        )
        .map_err(err)?;
        let res_topic = Topic::<LLMInferenceResult>::with_qos(
            participant.entity(),
            topics::LLM_RESULT,
            Some(&qos),
        )
        .map_err(err)?;
        let err_topic = Topic::<LLMInferenceError>::with_qos(
            participant.entity(),
            topics::LLM_ERROR,
            Some(&qos),
        )
        .map_err(err)?;

        Ok(Self {
            participant,
            publisher,
            subscriber,
            req_topic,
            res_topic,
            err_topic,
            agent_id,
        })
    }
}

impl Engine for DdsEngine {
    fn infer_stream(
        &self,
        req: InferRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<Chunk, EngineError>> + Send>> {
        let publisher = self.publisher.entity();
        let subscriber = self.subscriber.entity();
        let req_topic = self.req_topic.entity();
        let res_topic = self.res_topic.entity();
        let err_topic = self.err_topic.entity();
        let agent_id = self.agent_id.clone();

        Box::pin(stream! {
            use futures_util::StreamExt;

            let qos = match profiles::llm() {
                Ok(q) => q,
                Err(e) => {
                    yield Err(EngineError::DdsError(e.to_string()));
                    return;
                }
            };
            let req_writer = match DataWriter::with_qos(publisher, req_topic, Some(&qos)) {
                Ok(w) => w,
                Err(e) => {
                    yield Err(EngineError::DdsError(e.to_string()));
                    return;
                }
            };
            let res_reader = match DataReader::<LLMInferenceResult>::with_qos(subscriber, res_topic, Some(&qos)) {
                Ok(r) => r,
                Err(e) => {
                    yield Err(EngineError::DdsError(e.to_string()));
                    return;
                }
            };
            let err_reader = match DataReader::<LLMInferenceError>::with_qos(subscriber, err_topic, Some(&qos)) {
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
                provider_constraint: "ANY".into(),
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
