//! Construção do serviço sobre o DataSpace real (CycloneDDS) — feature `dds`.
//!
//! ## Nota sobre o tópico `ToolCall.Request` (desvio documentado da spec)
//!
//! `DataSpace::new(domain, strength)` já cria participant + os 17 tópicos
//! canônicos, INCLUINDO `ToolCall.Request` com `qos::profiles::tool_call()`
//! (Reliable 10s, TransientLocal, KeepLast 5, **Exclusive**) — perfil medido
//! na malha Python via SEDP (2026-07-17). Por isso este módulo NÃO cria um
//! segundo `Topic::<ToolCallRequest>` com `qos::profiles::llm()`:
//!
//! - um 2º `Topic` de mesmo nome e QoS diferente no MESMO participant é
//!   inconsistente para o CycloneDDS (falha na criação);
//! - `llm()` não seta ownership (default Shared) — e Shared não casa com os
//!   endpoints **Exclusive** da malha Python (ownership kind precisa ser igual
//!   no match reader/writer). Usar `llm()` quebraria a interop.
//!
//! O serviço usa `write_tool_call`/`subscribe_tool_calls` do próprio DataSpace.
//!
//! ## Strength do writer
//!
//! O gateway sobe com `STRENGTH_ORCHESTRATOR` (200): como o tópico é Exclusive
//! Ownership, o writer que grava o RESULTADO na instância criada pelo
//! requester precisa ter strength >= a dele (agente=100, cliente=10) — senão
//! as atualizações de status seriam filtradas pela arbitragem.

use crate::handler::ToolRegistry;
use crate::policy::PolicyHook;
use crate::service::ToolCallService;
use crate::tools::{external_tools, FilesystemTool};
use anyhow::{Context, Result};
use dds_dataspace::DataSpace;
use std::path::Path;
use std::sync::Arc;

/// Registry padrão do gateway: `filesystem.*` (raiz sandbox) + os 14 stubs
/// externos documentados (`github.*`, `web.*`, `database.*`, `cicd.*`).
pub fn default_registry(filesystem_root: impl AsRef<Path>) -> Result<ToolRegistry> {
    let registry = ToolRegistry::new();
    for tool in
        FilesystemTool::ops(filesystem_root).context("falha ao preparar a raiz do sandbox")?
    {
        registry.register(tool);
    }
    for stub in external_tools() {
        registry.register(stub);
    }
    Ok(registry)
}

/// Sobe o gateway completo: DataSpace no domínio (strength de orquestrador) +
/// registry padrão + política injetada. Pronto para `service.run()`.
pub fn build_service(
    domain_id: u32,
    filesystem_root: impl AsRef<Path>,
    policy: Arc<dyn PolicyHook>,
) -> Result<Arc<ToolCallService<DataSpace>>> {
    let data_space = DataSpace::new(domain_id, DataSpace::STRENGTH_ORCHESTRATOR)
        .context("falha ao subir o DataSpace")?;
    let registry = default_registry(filesystem_root)?;
    Ok(Arc::new(ToolCallService::new(data_space, registry, policy)))
}
