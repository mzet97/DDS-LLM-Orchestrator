//! Sobrecarga do gateway (REQ/T-820-17, sem DDS): com a fila de jobs cheia a
//! request NÃO é descartada silenciosamente — recebe FAILED na mesma
//! instância, sem claimar, e o exactly-once fica intacto.

use dds_contract::generated::dds_llm_orchestrator::ToolCallRequest;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::in_memory::InMemoryDataSpace;
use mcp_gateway::handler::ToolRegistry;
use mcp_gateway::service::{status, ToolCallService, OVERLOAD_MESSAGE};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn pending_call(call_id: &str) -> ToolCallRequest {
    ToolCallRequest {
        call_id: call_id.into(),
        request_id: "request".into(),
        requester_id: "agent-a".into(),
        tool_name: "test.append".into(),
        arguments_json: "{}".into(),
        status: status::PENDING,
        created_at_ns: now_ns(),
        ..Default::default()
    }
}

/// Caminho de sobrecarga: FAILED publicado na mesma instância com a mensagem
/// canônica, SEM criar claim — outra gateway ainda pode claimar a call
/// (o claim store da rejeição continua vazio para o call_id).
///
/// O acesso ao mock é via `service.data_space()` (mesma instância que o
/// serviço enxerga) — sem depender de `Clone` do `InMemoryDataSpace`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overload_publica_failed_sem_claim() {
    let service = ToolCallService::new(InMemoryDataSpace::new(), ToolRegistry::new());

    let call = pending_call("call-sobrecarga");
    service
        .data_space()
        .write_tool_call(call.clone())
        .await
        .unwrap();

    service.reject_overloaded(&call).await;

    let back = service
        .data_space()
        .read_tool_call("call-sobrecarga")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(back.status, status::FAILED);
    assert_eq!(back.error_message, OVERLOAD_MESSAGE);
    assert!(back.completed_at_ns > 0, "terminal com timestamp");
}

/// Guarda da rejeição: se a call já avançou em outra gateway (EXECUTING/
/// terminal), a recusa NÃO regrediu o estado visível.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overload_nao_regride_call_ja_avancada() {
    let service = ToolCallService::new(InMemoryDataSpace::new(), ToolRegistry::new());

    // Outra gateway já está executando (claimou e gravou EXECUTING).
    let mut call = pending_call("call-em-execucao");
    call.status = status::EXECUTING;
    service
        .data_space()
        .write_tool_call(call.clone())
        .await
        .unwrap();

    service.reject_overloaded(&call).await;

    let back = service
        .data_space()
        .read_tool_call("call-em-execucao")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        back.status,
        status::EXECUTING,
        "recusa não deve regredir o ciclo de outra gateway"
    );
    assert!(back.error_message.is_empty());
}
