//! `ToolHandler` (trait async object-safe) e `ToolRegistry` (dashmap de
//! handlers + despacho por `tool_name`).

use crate::error::ToolError;
use dashmap::DashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Futuro boxed de uma chamada de ferramenta.
///
/// A trait é object-safe sem depender de `async-trait`: cada handler retorna
/// o futuro pinado explicitamente (mesma ergonomia do `call_tool` Python).
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + 'a>>;

/// Handler de uma ferramenta exposta no barramento.
///
/// Paridade com os clientes Python (`MCPFilesystemClient.call_tool` etc.):
/// recebe os argumentos JSON crus e devolve o resultado como string (o serviço
/// embrulha em `{"result": ...}` ao gravar no DDS).
pub trait ToolHandler: Send + Sync {
    /// Nome canônico da ferramenta no tópico (ex.: `filesystem.read_file`).
    fn name(&self) -> &str;

    /// Executa a ferramenta com `arguments_json` (conteúdo do campo homônimo
    /// do `ToolCallRequest`).
    fn handle<'a>(&'a self, arguments_json: &'a str) -> ToolFuture<'a>;
}

/// Registry concorrente de handlers (dashmap — substitui o roteamento por
/// prefixo `_get_client` do Python: aqui cada tool name tem handler próprio).
#[derive(Default)]
pub struct ToolRegistry {
    handlers: DashMap<String, Arc<dyn ToolHandler>>,
}

impl ToolRegistry {
    /// Registry vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra um handler sob `handler.name()` (sobrescreve se já existir).
    pub fn register<H: ToolHandler + 'static>(&self, handler: H) {
        self.handlers
            .insert(handler.name().to_string(), Arc::new(handler));
    }

    /// Registra um handler já em `Arc` (útil para handlers compartilhados).
    pub fn register_arc(&self, handler: Arc<dyn ToolHandler>) {
        self.handlers.insert(handler.name().to_string(), handler);
    }

    /// `true` se existe handler para `tool_name`.
    pub fn contains(&self, tool_name: &str) -> bool {
        self.handlers.contains_key(tool_name)
    }

    /// Número de handlers registrados.
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// `true` se nenhum handler registrado.
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// Nomes das ferramentas registradas, ordenados (discovery, como o
    /// `list_tools()` dos clientes Python).
    pub fn list_tools(&self) -> Vec<String> {
        let mut names: Vec<String> = self.handlers.iter().map(|e| e.key().clone()).collect();
        names.sort();
        names
    }

    /// Despacha `arguments_json` para o handler de `tool_name`.
    ///
    /// `ToolError::UnknownTool` se não houver handler registrado.
    pub async fn dispatch(
        &self,
        tool_name: &str,
        arguments_json: &str,
    ) -> Result<String, ToolError> {
        // O guard do dashmap morre ANTES do await (clonamos o Arc e soltamos a
        // referência do shard) — segurar `get()` através de .await trava o shard.
        let handler = self.handlers.get(tool_name).map(|h| Arc::clone(h.value()));
        match handler {
            Some(h) => h.handle(arguments_json).await,
            None => Err(ToolError::UnknownTool(tool_name.to_string())),
        }
    }
}
