//! Parte de `machines` (extraída para manter os arquivos revisáveis).

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeStatus {
    pub state: ProbeState,
    /// Versão de protocolo anunciada ou motivo honesto da falha.
    pub detail: String,
}

/// Resultado do último probe por máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeState {
    /// Nó respondeu `GET /version` e o token validou em `/operations`.
    Online,
    /// Nó inalcançável ou resposta fora do contrato: estado honesto.
    Offline,
    /// Nó respondeu 401 (T-840-01): token ausente ou recusado.
    AuthPending,
    /// Nunca sondado (exibido como `?`).
    Unknown,
}

impl ProbeState {
    /// Ponto de status do painel: `●` online / `○` offline / `?` resto.
    #[must_use]
    pub fn dot(self) -> &'static str {
        match self {
            Self::Online => "●",
            Self::Offline => "○",
            Self::AuthPending | Self::Unknown => "?",
        }
    }
}

/// Erros do painel de máquinas (validação de formulário; probes são
/// reportados em [`ProbeStatus`], nunca como erro de thread).
#[derive(Debug, Error)]
pub enum MachinesError {
    /// `machine_id`/`node_url` vazios: nada a publicar.
    #[error("machine_id e node_url são obrigatórios")]
    MissingIdentity,
    /// Valor da máquina não serializou (serde).
    #[error("falha ao serializar máquina: {0}")]
    Serialize(String),
}

/// Mensagem do worker de máquinas.
pub(crate) enum MachinesMsg {
    /// Resposta de leitura do snapshot.
    Snapshot(Result<Snapshot, String>),
    /// Resposta de publicação + releitura do snapshot (mesma sequência do
    /// `catalog_remote`); `fill_base` preenche a base com a vigente após 409
    /// (T-840-03c: conflito → refresh + aviso).
    Mutated {
        notice: String,
        fill_base: Option<u64>,
        then_snapshot: Option<Result<Snapshot, String>>,
    },
    /// Resultado de um probe por `node_url` normalizado.
    Probed {
        node_url: String,
        status: ProbeStatus,
    },
}

/// Estado do painel de Máquinas (REQ/T-840-03c): snapshot da autoridade,
/// formulário de registro, tokens em memória e probes por máquina.
///
/// `Debug` manual de propósito: `tokens` e `form_token` nunca são impressos
/// (RNF-04 — o derive mostraria o segredo).
#[derive(Default)]
pub struct MachinesState {
    /// Autoridade do catálogo compartilhado (ex. `http://127.0.0.1:4317`).
    pub url: String,
    pub snapshot: Option<Snapshot>,
    /// Cursor do último snapshot aplicado (mesma semântica do `catalog_remote`).
    pub cursor: u64,
    pub notice: String,
    /// `true` enquanto há HTTP em background (`poll` drena e libera).
    pub busy: bool,
    // Formulário de registro.
    pub form_machine_id: String,
    pub form_host: String,
    pub form_user: String,
    pub form_node_url: String,
    /// Token do nó da máquina — SÓ memória local (vira `tokens` no publicar/
    /// guardar); nunca é enviado ao catálogo.
    pub form_token: String,
    pub form_services_hint: String,
    /// Base condicional do publish (vazia = criação; 409 preenche com a vigente).
    pub form_base: String,
    /// Token da autoridade do catálogo (T-840-03a) — SÓ memória local.
    pub authority_token: String,
    /// Tokens por `node_url` normalizado — SÓ memória local, morrem com o
    /// processo (RNF-04); nunca serializados nem persistidos.
    pub tokens: BTreeMap<String, String>,
    /// Último probe por `node_url` normalizado (linha da tabela).
    pub probes: BTreeMap<String, ProbeStatus>,
    /// Modal "Definir como Alvo & Configurar Token" (3.10): URL em edição.
    pub modal_url: Option<String>,
    /// Token digitado no modal (SÓ memória — RNF-04).
    pub modal_token: String,
    /// Sondas em voo (probe_all conta N; `poll` libera `busy` só no zero).
    pub(crate) pending_probes: usize,
    /// "Testar & Registrar": publica o formulário quando o probe em voo
    /// voltar `Online`; qualquer outro estado cancela com aviso.
    pub(crate) pending_publish_after_probe: bool,
    /// Token do formulário visível (alternador Exibir/Ocultar da 3.10).
    pub form_token_visible: bool,
    pub(crate) receiver: Option<mpsc::Receiver<MachinesMsg>>,
}

impl std::fmt::Debug for MachinesState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MachinesState")
            .field("url", &self.url)
            .field("snapshot", &self.snapshot)
            .field("cursor", &self.cursor)
            .field("notice", &self.notice)
            .field("busy", &self.busy)
            .field("form_machine_id", &self.form_machine_id)
            .field("form_host", &self.form_host)
            .field("form_user", &self.form_user)
            .field("form_node_url", &self.form_node_url)
            .field("form_token_definido", &!self.form_token.trim().is_empty())
            .field("form_services_hint", &self.form_services_hint)
            .field("form_base", &self.form_base)
            .field(
                "authority_token_definido",
                &!self.authority_token.trim().is_empty(),
            )
            .field("tokens(chaves)", &self.tokens.keys().collect::<Vec<_>>())
            .field("probes", &self.probes)
            .field("modal_url", &self.modal_url)
            .field("modal_token_definido", &!self.modal_token.trim().is_empty())
            .field("pending_probes", &self.pending_probes)
            .field(
                "pending_publish_after_probe",
                &self.pending_publish_after_probe,
            )
            .field("form_token_visible", &self.form_token_visible)
            .field("receiver", &self.receiver.is_some())
            .finish()
    }
}

/// Extrai o host de uma URL `esquema://host[:porta][/...]` (userinfo e
/// porta removidos; IPv6 com colchetes preservado). Usado para preencher o
/// registro quando o formulário traz só `node_url` (design 3.10).
pub(crate) fn host_from_url(node_url: &str) -> String {
    let after_scheme = node_url.split("://").nth(1).unwrap_or(node_url);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    if let Some(bracketed) = host_port.strip_prefix('[') {
        return bracketed.split(']').next().unwrap_or_default().to_string();
    }
    host_port.split(':').next().unwrap_or_default().to_string()
}

/// Normaliza a chave de `node_url` (trim + barra final removida).
pub(crate) fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// `Option<String>` só quando o texto não é vazio/branco.
pub(crate) fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Status honesto de um probe (worker → UI). 401 é distinto de offline:
/// "token ausente" quando a GUI não tinha token, "token recusado" quando
/// tinha (T-840-01; T-840-03c).
pub(crate) fn probe_status(url: &str, token: Option<&str>) -> ProbeStatus {
    match fetch_node_version_with_token(url, token) {
        Ok(version) => ProbeStatus {
            state: ProbeState::Online,
            detail: format!("protocolo {}.{}", version.major, version.minor),
        },
        Err(OriginError::Unauthorized) => ProbeStatus {
            state: ProbeState::AuthPending,
            detail: match token {
                Some(_) => String::from("token recusado (401)"),
                None => String::from("token ausente (nó respondeu 401)"),
            },
        },
        Err(err) => ProbeStatus {
            state: ProbeState::Offline,
            detail: err.to_string(),
        },
    }
}
