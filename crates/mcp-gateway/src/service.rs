//! `ToolCallService` — ciclo request→result no tópico `ToolCall.Request`.
//!
//! Port de `mcp_gateway/main.py` (classe `MCPGateway._process`): assina o
//! tópico, filtra `PENDING`, aplica o `PolicyHook`, despacha para o
//! `ToolRegistry` e grava cada transição de status NA MESMA instância
//! (chave = `call_id`).
//!
//! O serviço é genérico sobre `DataSpaceApi`: testes unitários usam
//! `InMemoryDataSpace` (sem CycloneDDS); com a feature `dds` ele roda sobre o
//! `DataSpace` real (ver `crate::dds`).

use crate::handler::ToolRegistry;
use crate::policy::PolicyHook;
use dds_contract::generated::dds_llm_orchestrator::ToolCallRequest;
use dds_dataspace::api::{DataSpaceApi, DataSpaceError};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// `ToolCallStatus` (espelha `models.py` Python / valores gravados no IDL).
pub mod status {
    /// Request aguardando processamento.
    pub const PENDING: i32 = 0;
    /// Permitido pela política (estado interno do Python, não gravado).
    pub const ALLOWED: i32 = 1;
    /// Negado pela política de segurança.
    pub const DENIED: i32 = 2;
    /// Em execução (gravado antes do dispatch, como no Python).
    pub const EXECUTING: i32 = 3;
    /// Concluído com sucesso (`result_json` preenchido).
    pub const COMPLETED: i32 = 4;
    /// Falhou (`error_message` preenchido).
    pub const FAILED: i32 = 5;
}

/// Mensagem de negação — byte-idêntica ao `main.py` Python.
pub const DENIED_MESSAGE: &str = "Negado pela politica de seguranca local";

/// Erro do serviço de tool calls.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// Falha de escrita/assinatura no DataSpace.
    #[error("dataspace: {0}")]
    DataSpace(#[from] DataSpaceError),
}

/// Relógio em ns (mesma unidade dos campos `*_at_ns` do IDL).
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Stream boxed de `ToolCallRequest` (retorno de `subscribe_tool_calls`).
pub type ToolCallStream =
    std::pin::Pin<Box<dyn futures_core::Stream<Item = ToolCallRequest> + Send>>;

/// Consome o próximo item da stream usando só `std::future::poll_fn`
/// (evita a dependência `futures-util` só para `StreamExt::next`).
pub async fn next_tool_call(stream: &mut ToolCallStream) -> Option<ToolCallRequest> {
    std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
}

/// O MCP Gateway propriamente dito: política + registry + loop DDS.
pub struct ToolCallService<D: DataSpaceApi> {
    data_space: D,
    registry: ToolRegistry,
    policy: Arc<dyn PolicyHook>,
}

impl<D: DataSpaceApi + 'static> ToolCallService<D> {
    /// Cria o serviço com DataSpace, registry de ferramentas e política.
    pub fn new(data_space: D, registry: ToolRegistry, policy: Arc<dyn PolicyHook>) -> Self {
        Self {
            data_space,
            registry,
            policy,
        }
    }

    /// Registry de ferramentas.
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// DataSpace subjacente.
    pub fn data_space(&self) -> &D {
        &self.data_space
    }

    /// Processa UM `ToolCall.Request` (ciclo completo, paridade com o Python):
    ///
    /// 1. Política nega → `DENIED` + [`DENIED_MESSAGE`], grava e retorna.
    /// 2. Grava `EXECUTING` na mesma instância.
    /// 3. Dispatch: sucesso → `COMPLETED` + `result_json = {"result": ...}`;
    ///    erro → `FAILED` + `error_message`.
    /// 4. Grava o estado terminal com `completed_at_ns` estampado.
    ///
    /// Retorna o estado final gravado.
    pub async fn process_one(
        &self,
        request: &ToolCallRequest,
    ) -> Result<ToolCallRequest, ServiceError> {
        let mut call = request.clone();
        tracing::info!(
            call_id = %call.call_id,
            tool = %call.tool_name,
            "ToolCall recebido"
        );

        // 1. Governança (fast path local — policy.evaluate do Python).
        if !self
            .policy
            .check(&call.tool_name, call.security_level, &call.arguments_json)
        {
            call.status = status::DENIED;
            call.error_message = DENIED_MESSAGE.to_string();
            call.completed_at_ns = now_ns();
            self.data_space.write_tool_call(call.clone()).await?;
            tracing::warn!(call_id = %call.call_id, tool = %call.tool_name, "ToolCall negado pela politica local");
            return Ok(call);
        }

        // 2. EXECUTING visível no mesh antes do dispatch (como o Python).
        call.status = status::EXECUTING;
        self.data_space.write_tool_call(call.clone()).await?;

        // 3. Dispatch para o handler registrado.
        match self
            .registry
            .dispatch(&call.tool_name, &call.arguments_json)
            .await
        {
            Ok(result) => {
                call.status = status::COMPLETED;
                // Python: request.result_json = json.dumps({"result": result})
                call.result_json = serde_json::json!({ "result": result }).to_string();
                tracing::info!(call_id = %call.call_id, tool = %call.tool_name, "ToolCall completado");
            }
            Err(e) => {
                call.status = status::FAILED;
                // Python: request.error_message = str(e)
                call.error_message = e.to_string();
                tracing::warn!(call_id = %call.call_id, tool = %call.tool_name, error = %e, "ToolCall falhou");
            }
        }

        // 4. Grava o estado terminal na MESMA instância (chave = call_id).
        call.completed_at_ns = now_ns();
        self.data_space.write_tool_call(call.clone()).await?;
        Ok(call)
    }

    /// Loop principal (`MCPGateway.run` do Python): assina `ToolCall.Request`,
    /// filtra `status == PENDING` e processa cada request em task tokio
    /// própria (o `asyncio.run_coroutine_threadsafe` do Python).
    ///
    /// Retorna quando a stream fecha (shutdown do DataSpace).
    pub async fn run(self: Arc<Self>) -> Result<(), ServiceError> {
        let mut stream = self.data_space.subscribe_tool_calls();
        tracing::info!("mcp-gateway: aguardando ToolCall.Request (status=PENDING)");

        while let Some(request) = next_tool_call(&mut stream).await {
            // Eco das próprias escritas (EXECUTING/COMPLETED/...) é ignorado,
            // como o filtro `request.status != PENDING: return` do Python.
            if request.status != status::PENDING {
                continue;
            }
            let svc = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = svc.process_one(&request).await {
                    tracing::error!(call_id = %request.call_id, error = %e, "falha ao processar ToolCall");
                }
            });
        }
        Ok(())
    }
}
