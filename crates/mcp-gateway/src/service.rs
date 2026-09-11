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

use crate::claim::{ClaimDecision, ClaimError, ClaimStore, MemoryClaimStore, OwnerId};
use crate::handler::ToolRegistry;
use crate::policy::{DistributedPolicy, PolicyDecision};
use dds_contract::generated::dds_llm_orchestrator::{
    SecurityPolicySnapshot, SecurityPolicyUpdate, ToolCallRequest,
};
use dds_dataspace::api::{DataSpaceApi, DataSpaceError};
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// A persistent ownership claim failed.
    #[error("claim: {0}")]
    Claim(#[from] ClaimError),
    /// Another gateway already owns this call; dispatch is forbidden.
    #[error("call_id already claimed")]
    AlreadyClaimed,
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
    policy: Arc<DistributedPolicy>,
    claims: Arc<dyn ClaimStore>,
    owner: OwnerId,
}

impl<D: DataSpaceApi + 'static> ToolCallService<D> {
    /// Cria o serviço com DataSpace, registry de ferramentas e política.
    pub fn new(data_space: D, registry: ToolRegistry) -> Self {
        Self::with_policy(data_space, registry, Arc::new(DistributedPolicy::default()))
    }

    pub fn with_policy(
        data_space: D,
        registry: ToolRegistry,
        policy: Arc<DistributedPolicy>,
    ) -> Self {
        Self::with_policy_and_claims(
            data_space,
            registry,
            policy,
            Arc::new(MemoryClaimStore::default()),
            next_owner_id(),
        )
    }

    /// Creates a service with an explicit cross-instance claim store (REQ-706).
    pub fn with_policy_and_claims(
        data_space: D,
        registry: ToolRegistry,
        policy: Arc<DistributedPolicy>,
        claims: Arc<dyn ClaimStore>,
        owner: OwnerId,
    ) -> Self {
        Self {
            data_space,
            registry,
            policy,
            claims,
            owner,
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

    pub fn policy(&self) -> &DistributedPolicy {
        &self.policy
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
        let version = match self.policy.evaluate(&call) {
            PolicyDecision::Denied { reason } => {
                call.status = status::DENIED;
                call.error_message = DENIED_MESSAGE.to_string();
                call.completed_at_ns = now_ns();
                self.data_space.write_tool_call(call.clone()).await?;
                tracing::warn!(
                    audit = true,
                    call_id = %call.call_id,
                    tool = %call.tool_name,
                    security_level = call.security_level,
                    requester_id_len = call.requester_id.len(),
                    decision = "deny",
                    reason = reason.as_str(),
                    "MCP policy decision"
                );
                return Ok(call);
            }
            PolicyDecision::Allowed { version } => version,
        };
        tracing::info!(
            audit = true,
            call_id = %call.call_id,
            tool = %call.tool_name,
            security_level = call.security_level,
            requester_id_len = call.requester_id.len(),
            policy_version = version,
            decision = "allow",
            "MCP policy decision"
        );

        match self.claims.try_claim(&call.call_id, &self.owner)? {
            ClaimDecision::Won => {}
            ClaimDecision::AlreadyClaimed => return Err(ServiceError::AlreadyClaimed),
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
        let mut tool_calls = self.data_space.subscribe_tool_calls();
        let mut snapshots = self.data_space.subscribe_security_snapshots();
        let mut updates = self.data_space.subscribe_security_updates();
        let mut jobs = tokio::task::JoinSet::new();
        tracing::info!("mcp-gateway: aguardando ToolCall.Request (status=PENDING)");

        loop {
            tokio::select! {
                maybe_request = next_tool_call(&mut tool_calls), if jobs.len() < 64 => {
                    let Some(request) = maybe_request else { break };
                    if request.status == status::PENDING {
                        let svc = Arc::clone(&self);
                        jobs.spawn(async move { svc.process_one(&request).await });
                    }
                }
                maybe_snapshot = next_security_snapshot(&mut snapshots) => {
                    let Some(snapshot) = maybe_snapshot else { break };
                    self.accept_snapshot(&snapshot);
                }
                maybe_update = next_security_update(&mut updates) => {
                    let Some(update) = maybe_update else { break };
                    self.accept_update(&update);
                }
                Some(joined) = jobs.join_next(), if !jobs.is_empty() => {
                    match joined {
                        Ok(Ok(_)) => {}
                        Ok(Err(error)) => tracing::error!(error = %error, "ToolCall processing failed"),
                        Err(error) => tracing::error!(error = %error, "ToolCall task failed"),
                    }
                }
            }
        }
        while let Some(joined) = jobs.join_next().await {
            match joined {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => tracing::error!(error = %error, "ToolCall processing failed"),
                Err(error) => tracing::error!(error = %error, "ToolCall task failed"),
            }
        }
        Ok(())
    }

    fn accept_snapshot(&self, snapshot: &SecurityPolicySnapshot) {
        match self.policy.ingest_snapshot(snapshot) {
            Ok(()) => tracing::info!(
                audit = true,
                policy_id = %snapshot.policy_id,
                version = snapshot.version,
                decision = "accept",
                "MCP policy snapshot"
            ),
            Err(error) => tracing::warn!(
                audit = true,
                policy_id = %snapshot.policy_id,
                version = snapshot.version,
                decision = "reject",
                reason = %error,
                "MCP policy snapshot"
            ),
        }
    }

    fn accept_update(&self, update: &SecurityPolicyUpdate) {
        match self.policy.ingest_update(update) {
            Ok(()) => tracing::info!(
                audit = true,
                policy_id = %update.policy_id,
                version = update.new_version,
                decision = "accept",
                "MCP policy update"
            ),
            Err(error) => tracing::warn!(
                audit = true,
                policy_id = %update.policy_id,
                version = update.new_version,
                decision = "reject",
                reason = %error,
                "MCP policy update"
            ),
        }
    }
}

fn next_owner_id() -> OwnerId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    OwnerId::generated(std::process::id(), sequence)
}

type SecuritySnapshotStream =
    std::pin::Pin<Box<dyn futures_core::Stream<Item = SecurityPolicySnapshot> + Send>>;
type SecurityUpdateStream =
    std::pin::Pin<Box<dyn futures_core::Stream<Item = SecurityPolicyUpdate> + Send>>;

async fn next_security_snapshot(
    stream: &mut SecuritySnapshotStream,
) -> Option<SecurityPolicySnapshot> {
    std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
}

async fn next_security_update(stream: &mut SecurityUpdateStream) -> Option<SecurityPolicyUpdate> {
    std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
}
