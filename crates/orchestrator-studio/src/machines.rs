//! Máquinas (REQ/T-840-03, painel "Máquinas" do Studio multi-host).
//!
//! Registro de nós remotos (`studio-node` em outros hosts) no CATÁLOGO
//! COMPARTILHADO (SDD §34): várias GUIs leem a mesma lista de máquinas pela
//! autoridade; o catálogo é genérico sobre o valor (`publish(id, base,
//! value, generation)`), então nada muda no schema de `studio-core`.
//!
//! REGRA DE SEGREDO (RNF-04 / SDD §28, deliberada): o token do nó NUNCA vai
//! para o catálogo nem para o journal. [`MachineRecord`] não tem campo de
//! segredo — o valor publicado é só o envelope `{"kind":"machine",
//! "machine":{...}}`. Tokens vivem SÓ na memória da GUI
//! ([`MachinesState::tokens`], chaveados por `node_url`) e morrem com o
//! processo; o `Debug` manual de [`MachinesState`] é proposital para nunca
//! imprimi-los. Um probe 401 (nó conforme T-840-01) vira a dica honesta
//! "token ausente/recusado" — nunca retry silencioso.
//!
//! HTTP bloqueante roda em THREAD de trabalho (padrão `catalog_remote.rs`/
//! `agents.rs`: thread + mpsc + `poll` por frame — REQ/T-820-19); a thread
//! de UI nunca bloqueia.

use std::collections::BTreeMap;
use std::sync::mpsc;

use serde::{Deserialize, Serialize};
use studio_core::catalog::Snapshot;
use thiserror::Error;

use crate::catalog_remote::{fetch_snapshot_with_token, publish_with_token, SharedCatalogError};
use crate::origin::{fetch_node_version_with_token, optional_token, OriginError};

/// Marcador de tipo no valor do catálogo: só valores com
/// `"kind":"machine"` são máquinas (outras entradas do catálogo não
/// colidem com o painel).
pub const MACHINE_KIND: &str = "machine";

/// Prefixo do id no catálogo: `machine:{machine_id}` (G-52: identidade
/// explícita, nunca fundir por nome com outros objetos).
pub const MACHINE_ID_PREFIX: &str = "machine:";

/// Máquina registrada no catálogo compartilhado (REQ/T-840-03b).
///
/// SEM campo de token por construção: segredo fica por referência
/// (RNF-04) — quem sondar a máquina informa o token da memória local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineRecord {
    /// Identidade escolhida pelo usuário ou derivada do host (ex. `orch-62`).
    pub machine_id: String,
    /// IP ou hostname da máquina.
    pub host: String,
    /// Usuário de acesso (para SSH/operação fora da GUI).
    pub user: String,
    /// URL do `studio-node` remoto (ex. `http://192.168.1.61:4317`).
    pub node_url: String,
    /// Dica opcional de serviços próprios (CSV), conforme o nó anuncia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub services_hint: Option<String>,
    /// Momento do registro (UNIX ns) — rastreio, não lógica.
    pub registered_at_ns: u64,
}

/// Envelope de fio: `kind` separa máquinas de outros objetos do catálogo
/// (T-840-03b). [`parse_machine_value`] rejeita o que não é `machine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineEnvelope {
    pub kind: String,
    pub machine: MachineRecord,
}

impl MachineRecord {
    /// Id desta máquina no catálogo: `machine:{machine_id}`.
    #[must_use]
    pub fn catalog_id(&self) -> String {
        format!("{MACHINE_ID_PREFIX}{}", self.machine_id)
    }
}

/// Serializa a máquina para o valor do catálogo, com o envelope `kind`
/// (T-840-03b). O TOKEN deliberadamente não existe no registro.
///
/// # Errors
/// Erro de serialização serde (na prática infalível para este tipo, mas o
/// aviso honesto é melhor que silenciar).
pub fn machine_catalog_value(machine: &MachineRecord) -> Result<String, serde_json::Error> {
    serde_json::to_string(&MachineEnvelope {
        kind: String::from(MACHINE_KIND),
        machine: machine.clone(),
    })
}

/// Converte um valor do catálogo em máquina: exige `kind == "machine"`
/// (T-840-03b) — entradas de outros objetos (ou lixo) são recusadas.
#[must_use]
pub fn parse_machine_value(value: &str) -> Option<MachineRecord> {
    let envelope: MachineEnvelope = serde_json::from_str(value).ok()?;
    (envelope.kind == MACHINE_KIND).then_some(envelope.machine)
}

/// Máquinas do snapshot, ordenadas por `machine_id` (visual estável).
#[must_use]
pub fn machines_from_snapshot(snapshot: &Snapshot) -> Vec<MachineRecord> {
    let mut machines: Vec<MachineRecord> = snapshot
        .items
        .iter()
        .filter_map(|item| parse_machine_value(&item.value))
        .collect();
    machines.sort_by(|a, b| a.machine_id.cmp(&b.machine_id));
    machines
}

/// Momento atual em UNIX ns (0 se o relógio estiver antes da época —
/// honesto para um campo de rastreio).
#[must_use]
pub fn now_unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// Estado da sonda de uma máquina (linha da tabela do painel).
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
enum MachinesMsg {
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
    pending_probes: usize,
    /// "Testar & Registrar": publica o formulário quando o probe em voo
    /// voltar `Online`; qualquer outro estado cancela com aviso.
    pending_publish_after_probe: bool,
    /// Token do formulário visível (alternador Exibir/Ocultar da 3.10).
    pub form_token_visible: bool,
    receiver: Option<mpsc::Receiver<MachinesMsg>>,
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
fn host_from_url(node_url: &str) -> String {
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
fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Status honesto de um probe (worker → UI). 401 é distinto de offline:
/// "token ausente" quando a GUI não tinha token, "token recusado" quando
/// tinha (T-840-01; T-840-03c).
fn probe_status(url: &str, token: Option<&str>) -> ProbeStatus {
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

impl MachinesState {
    /// Padrão honesto: autoridade local, sem snapshot nem tokens.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        Self {
            url: String::from(url),
            ..Self::default()
        }
    }

    /// Token efetivo da autoridade: campo explícito, ou o token em memória
    /// guardado para a URL da autoridade (modal/formulário da 3.10), ou o
    /// token de deploy em disco (mesma fonte do painel Nó/auto-carga) — a
    /// autoridade é o alvo global (`main` sincroniza `url` a cada frame).
    fn auth(&self) -> Option<String> {
        if let Some(token) = optional_token(&self.authority_token) {
            return Some(token.to_string());
        }
        let normalized = normalize_url(&self.url);
        if let Some(token) = self.tokens.get(&normalized) {
            return Some(token.clone());
        }
        crate::discovery::token_for_url(&self.url, std::env::var("HOME").ok().as_deref())
    }

    /// Token em memória da máquina (chave normalizada) — nunca sai da GUI.
    #[must_use]
    pub fn token_for(&self, node_url: &str) -> Option<&str> {
        self.tokens
            .get(&normalize_url(node_url))
            .map(String::as_str)
    }

    /// URLs sondadas com `AuthPending` e SEM token em memória — a dica
    /// "token ausente" do painel (T-840-03c).
    #[must_use]
    pub fn missing_token_urls(&self) -> Vec<String> {
        self.probes
            .iter()
            .filter(|(url, status)| {
                status.state == ProbeState::AuthPending && !self.tokens.contains_key(*url)
            })
            .map(|(url, _)| url.clone())
            .collect()
    }

    /// Máquinas do snapshot corrente, ordenadas (só `kind=="machine"`).
    #[must_use]
    pub fn machines(&self) -> Vec<MachineRecord> {
        self.snapshot
            .as_ref()
            .map(machines_from_snapshot)
            .unwrap_or_default()
    }

    /// Relê o snapshot da autoridade em background. Clique durante `busy`
    /// é ignorado (mesma disciplina do `catalog_remote`).
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let auth = self.auth();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(MachinesMsg::Snapshot(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("lendo snapshot…");
    }

    /// Guarda o token do formulário na memória local, chaveado pelo
    /// `node_url` do formulário (T-840-03c). Token vazio REMOVE a entrada.
    /// NUNCA vai ao catálogo nem a disco (RNF-04).
    pub fn save_token_from_form(&mut self) {
        let url = normalize_url(&self.form_node_url);
        if url.is_empty() {
            self.notice = String::from("node_url é obrigatório para guardar o token");
            return;
        }
        match optional_token(&self.form_token) {
            Some(token) => {
                self.tokens.insert(url.clone(), String::from(token));
                self.notice =
                    format!("token de {url} guardado só na memória (nunca vai ao catálogo)");
            }
            None => {
                if self.tokens.remove(&url).is_some() {
                    self.notice = format!("token de {url} removido da memória");
                } else {
                    self.notice = String::from("nenhum token para guardar");
                }
            }
        }
    }

    /// Publica a máquina do formulário no catálogo compartilhado (fluxo do
    /// `SharedCatalog`: publish condicional por base + releitura do
    /// snapshot). Conflito 409 (T-840-03c): o snapshot é relido, o aviso
    /// mostra a vigente e a base do formulário é preenchida — nada é
    /// aplicado silenciosamente. Se o formulário tem token, ele vai SÓ para
    /// a memória local (`tokens`), nunca para o valor publicado (RNF-04).
    ///
    /// # Errors
    /// Validação/serialização falham ANTES de any worker: `notice` registra
    /// o motivo e nada é publicado (retorna `Err` para o chamador decidir).
    pub fn publish_machine(&mut self) -> Result<(), MachinesError> {
        if self.busy {
            return Ok(());
        }
        let node_url = normalize_url(&self.form_node_url);
        let machine_id = self.form_machine_id.trim().to_string();
        if machine_id.is_empty() || node_url.is_empty() {
            self.notice = String::from("machine_id e node_url são obrigatórios");
            return Err(MachinesError::MissingIdentity);
        }
        if let Some(token) = optional_token(&self.form_token) {
            self.tokens.insert(node_url.clone(), String::from(token));
        }
        let host = {
            let explicit = self.form_host.trim();
            if explicit.is_empty() {
                // Design 3.10: o formulário traz só alias + URL — o host é
                // derivado da URL real (nada inventado).
                host_from_url(&node_url)
            } else {
                explicit.to_string()
            }
        };
        let machine = MachineRecord {
            machine_id,
            host,
            user: self.form_user.trim().to_string(),
            node_url,
            services_hint: non_empty(&self.form_services_hint),
            registered_at_ns: now_unix_ns(),
        };
        let value = machine_catalog_value(&machine).map_err(|err| {
            self.notice = format!("falha ao serializar máquina: {err}");
            MachinesError::Serialize(err.to_string())
        })?;
        let id = machine.catalog_id();
        let base = self.form_base.trim().parse::<u64>().ok();
        let url = self.url.clone();
        let auth = self.auth();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (notice, fill_base) =
                match publish_with_token(&url, &id, base, &value, auth.as_deref()) {
                    Ok(revision) => (format!("publicado em r{revision}"), None),
                    Err(err) => (
                        err.to_string(),
                        match &err {
                            SharedCatalogError::Conflict {
                                current: Some(current),
                            } => Some(*current),
                            _ => None,
                        },
                    ),
                };
            let then_snapshot = Some(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            );
            let _ = tx.send(MachinesMsg::Mutated {
                notice,
                fill_base,
                then_snapshot,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = format!("publicando máquina {}…", machine.machine_id);
        Ok(())
    }

    /// Sonda a máquina (`GET {node_url}/version`, T-840-03c) em background,
    /// com o token em memória para aquele `node_url` — se houver. Mapa vazio
    /// no arranque → nós protegidos respondem 401 → dica "token ausente" com
    /// o formulário para (re)entrar. Clique durante `busy` é ignorado.
    pub fn probe(&mut self, machine: &MachineRecord) {
        self.probe_url(&machine.node_url);
        if self.busy {
            self.notice = format!("sondando {}…", machine.machine_id);
        }
    }

    /// Sonda uma URL descoberta (mesma sonda do catálogo; usada pelo botão
    /// "Forçar probe"/"Re-testar" da tabela 3.10). Clique durante `busy` é
    /// ignorado.
    pub fn probe_url(&mut self, node_url: &str) {
        if self.busy {
            return;
        }
        let url = normalize_url(node_url);
        let token = self.tokens.get(&url).cloned();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let status = probe_status(&url, token.as_deref());
            let _ = tx.send(MachinesMsg::Probed {
                node_url: url,
                status,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.pending_probes = 1;
        self.notice = String::from("sondando…");
    }

    /// Sonda TODAS as URLs descobertas (botão "Forçar Probe" da 3.10): uma
    /// thread por URL no mesmo canal; `poll` drena e só libera `busy` quando
    /// a última responder. Clique durante `busy` ou lista vazia é ignorado.
    pub fn probe_all(&mut self, node_urls: &[String]) {
        if self.busy || node_urls.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        for node_url in node_urls {
            let url = normalize_url(node_url);
            let token = self.tokens.get(&url).cloned();
            let tx = tx.clone();
            std::thread::spawn(move || {
                let status = probe_status(&url, token.as_deref());
                let _ = tx.send(MachinesMsg::Probed {
                    node_url: url,
                    status,
                });
            });
        }
        self.receiver = Some(rx);
        self.busy = true;
        self.pending_probes = node_urls.len();
        self.notice = format!("sondando {} nó(s)…", node_urls.len());
    }

    /// "Testar & Registrar" (design 3.10): valida o formulário com um probe
    /// imediato e SÓ publica no catálogo se voltar `Online`. Exige
    /// `machine_id` + `node_url` (mesma regra do publicar direto).
    pub fn test_and_register(&mut self) {
        if self.busy {
            return;
        }
        let node_url = normalize_url(&self.form_node_url);
        if self.form_machine_id.trim().is_empty() || node_url.is_empty() {
            self.notice = String::from("machine_id e node_url são obrigatórios");
            return;
        }
        if let Some(token) = optional_token(&self.form_token) {
            self.tokens.insert(node_url.clone(), String::from(token));
        }
        self.pending_publish_after_probe = true;
        self.probe_url(&node_url);
        self.notice = format!("validando {node_url} antes de registrar…");
    }

    /// Limpa o formulário de registro manual (botão "Limpar" da 3.10).
    pub fn clear_form(&mut self) {
        self.form_machine_id.clear();
        self.form_node_url.clear();
        self.form_token.clear();
    }

    /// Guarda token de uma URL descoberta na memória local (modal 3.10
    /// "Definir como Alvo & Configurar Token"). NUNCA vai ao catálogo nem a
    /// disco (RNF-04).
    pub fn remember_token(&mut self, node_url: &str, token: &str) {
        let url = normalize_url(node_url);
        let trimmed = token.trim();
        if trimmed.is_empty() {
            self.tokens.remove(&url);
        } else {
            self.tokens.insert(url, String::from(trimmed));
        }
    }

    /// Drena o worker; chamar a cada frame enquanto `busy` (padrão
    /// `catalog_remote::poll` — nota idêntica: dentro de `poll` o snapshot é
    /// aplicado por atribuição direta de campos).
    pub fn poll(&mut self) {
        let mut finished = false;
        // Drena para um Vec antes de processar: os braços podem rearmar
        // `busy`/`receiver` (encadeamento do "Testar & Registrar") sem
        // conflitar com o borrow do canal.
        let pending: Vec<MachinesMsg> = self
            .receiver
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for msg in pending {
            match msg {
                MachinesMsg::Snapshot(result) => {
                    match result {
                        Ok(snapshot) => {
                            self.cursor = snapshot.cursor.0;
                            self.snapshot = Some(snapshot);
                            self.notice.clear();
                        }
                        Err(err) => {
                            self.notice = err;
                        }
                    }
                    finished = true;
                }
                MachinesMsg::Mutated {
                    notice,
                    fill_base,
                    then_snapshot,
                } => {
                    if let Some(current) = fill_base {
                        // 409: base do formulário preenchida com a vigente
                        // para o próximo clique (T-840-03c).
                        self.form_base = current.to_string();
                        self.notice = format!("{notice}; base preenchida com a vigente");
                    } else {
                        self.notice = notice;
                    }
                    if let Some(result) = then_snapshot {
                        match result {
                            Ok(snapshot) => {
                                self.cursor = snapshot.cursor.0;
                                self.snapshot = Some(snapshot);
                            }
                            Err(err) => {
                                self.notice = err;
                            }
                        }
                    }
                    finished = true;
                }
                MachinesMsg::Probed { node_url, status } => {
                    let online = status.state == ProbeState::Online;
                    let detail = status.detail.clone();
                    self.notice = detail.clone();
                    self.probes.insert(node_url.clone(), status);
                    self.pending_probes = self.pending_probes.saturating_sub(1);
                    if self.pending_publish_after_probe {
                        // "Testar & Registrar": Online publica, resto cancela.
                        self.pending_publish_after_probe = false;
                        if online {
                            self.busy = false;
                            self.receiver = None;
                            let _ = self.publish_machine();
                        } else {
                            self.notice = format!(
                                "probe de {node_url} não passou ({detail}); registro NÃO \
                                     persistido"
                            );
                            if self.pending_probes == 0 {
                                finished = true;
                            }
                        }
                    } else if self.pending_probes == 0 {
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_core::catalog::{Cursor, DefinitionId, ItemView};
    use studio_core::revision::Revision;

    fn machine(id: &str) -> MachineRecord {
        MachineRecord {
            machine_id: String::from(id),
            host: String::from("192.168.1.61"),
            user: String::from("tese"),
            node_url: String::from("http://192.168.1.61:4317"),
            services_hint: Some(String::from("studio-noded,orchestrator")),
            registered_at_ns: 1_759_500_000_000_000_000,
        }
    }

    #[test]
    fn machine_record_round_trips_through_catalog_value() {
        // Given: máquina registrada; When: valor de catálogo ida-e-volta.
        let original = machine("orch-62");
        let value = machine_catalog_value(&original).expect("serializa");
        let parsed = parse_machine_value(&value).expect("envelope machine");
        // Then: registro idêntico; envelope expõe kind; id prefixado.
        assert_eq!(parsed, original);
        assert!(value.contains(r#""kind":"machine""#));
        assert!(!value.contains("token"), "segredo não existe no registro");
        assert_eq!(original.catalog_id(), "machine:orch-62");
    }

    #[test]
    fn parse_rejects_non_machine_entries() {
        // Entradas de outros objetos do catálogo (e lixo) nunca viram máquina.
        assert!(parse_machine_value(r#"{"kind":"agent","agent_id":"a"}"#).is_none());
        assert!(parse_machine_value("texto solto").is_none());
        assert!(parse_machine_value(r#"{"machine":{"machine_id":"x"}}"#).is_none());
        assert!(parse_machine_value("{}").is_none());
    }

    #[test]
    fn machines_from_snapshot_filters_only_machines_and_sorts() {
        let machine_a = machine("orch-b");
        let machine_b = machine("orch-a");
        let snapshot = Snapshot {
            items: vec![
                ItemView {
                    id: DefinitionId(String::from("proj-z")),
                    value: String::from(r#"{"kind":"agent"}"#),
                    revision: Revision(0),
                },
                ItemView {
                    id: DefinitionId(machine_b.catalog_id()),
                    value: machine_catalog_value(&machine_b).expect("serializa"),
                    revision: Revision(0),
                },
                ItemView {
                    id: DefinitionId(machine_a.catalog_id()),
                    value: machine_catalog_value(&machine_a).expect("serializa"),
                    revision: Revision(3),
                },
                ItemView {
                    id: DefinitionId(String::from("machine:quebrada")),
                    value: String::from("{não é json"),
                    revision: Revision(1),
                },
            ],
            cursor: Cursor(4),
        };
        let machines = machines_from_snapshot(&snapshot);
        assert_eq!(
            machines
                .iter()
                .map(|m| m.machine_id.as_str())
                .collect::<Vec<_>>(),
            vec!["orch-a", "orch-b"],
            "só kind==machine, ordenado por machine_id"
        );
    }

    #[test]
    fn debug_never_contains_tokens() {
        // RNF-04: o Debug do estado não pode vazar segredo (nem valor, nem
        // presença por chave de token).
        let mut state = MachinesState::with_url("http://127.0.0.1:4317");
        state.form_token = String::from("segredo-super-forte-123");
        state.authority_token = String::from("autoridade-secreta-456");
        state.tokens.insert(
            String::from("http://192.168.1.61:4317"),
            String::from("outro-segredo-789"),
        );
        let rendered = format!("{state:?}");
        assert!(
            !rendered.contains("segredo"),
            "Debug vazou token: {rendered}"
        );
        assert!(
            !rendered.contains("autoridade"),
            "Debug vazou token: {rendered}"
        );
    }

    #[test]
    fn probe_unreachable_is_offline_honest() {
        // Porta fechada no loopback: recusa determinística de conexão.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback deve ligar");
        let port = listener.local_addr().expect("porta legivel").port();
        drop(listener);
        let status = probe_status(&format!("http://127.0.0.1:{port}"), None);
        assert_eq!(status.state, ProbeState::Offline);
        assert!(
            status.detail.contains("falha ao ler o no"),
            "{}",
            status.detail
        );
    }

    #[test]
    fn default_state_starts_with_empty_tokens_and_unknown_probes() {
        // Arranque honesto: sem tokens em memória, nenhuma sonda feita.
        let state = MachinesState::with_url("http://127.0.0.1:4317");
        assert!(state.tokens.is_empty());
        assert!(state.probes.is_empty());
        assert!(state.machines().is_empty());
        assert!(state.missing_token_urls().is_empty());
        assert_eq!(ProbeState::Unknown.dot(), "?");
    }

    #[test]
    fn probe_all_ignores_empty_and_counts_pending() {
        // Given: painel ocioso.
        let mut state = MachinesState::with_url("http://127.0.0.1:1");
        // When: lista vazia.
        state.probe_all(&[]);
        // Then: nada acontece.
        assert!(!state.busy);
        assert_eq!(state.pending_probes, 0);
        // When: duas URLs (loopback fechado = falha rápida determinística).
        state.probe_all(&[
            String::from("http://127.0.0.1:1"),
            String::from("http://127.0.0.1:2"),
        ]);
        // Then: ocupado com 2 sondas em voo.
        assert!(state.busy);
        assert_eq!(state.pending_probes, 2);
    }

    #[test]
    fn poll_releases_busy_only_on_last_probe() {
        // Given: 2 sondas em voo com canal injetado (sem rede).
        let mut state = MachinesState::with_url("http://127.0.0.1:1");
        let (tx, rx) = mpsc::channel();
        state.receiver = Some(rx);
        state.busy = true;
        state.pending_probes = 2;
        let probed = |url: &str| MachinesMsg::Probed {
            node_url: String::from(url),
            status: ProbeStatus {
                state: ProbeState::Offline,
                detail: String::from("fechado"),
            },
        };
        // When: primeira resposta.
        tx.send(probed("http://a:4317")).expect("canal vivo");
        state.poll();
        // Then: ainda ocupado (falta 1).
        assert_eq!(state.pending_probes, 1);
        assert!(state.busy);
        assert!(state.receiver.is_some());
        // When: última resposta.
        tx.send(probed("http://b:4317")).expect("canal vivo");
        state.poll();
        // Then: liberado com as 2 sondas aplicadas.
        assert_eq!(state.pending_probes, 0);
        assert!(!state.busy);
        assert_eq!(state.probes.len(), 2);
    }

    #[test]
    fn test_and_register_requires_identity() {
        // Given: formulário vazio.
        let mut state = MachinesState::with_url("http://127.0.0.1:1");
        // When: Testar & Registrar sem identidade.
        state.test_and_register();
        // Then: aviso, sem sonda.
        assert!(!state.busy);
        assert!(state.notice.contains("obrigatórios"));
    }

    #[test]
    fn publish_after_probe_chains_only_on_online() {
        // Given: formulário válido + probe em voo com publicação pendente.
        let mut state = MachinesState::with_url("http://127.0.0.1:1");
        state.form_machine_id = String::from("novo-70");
        state.form_node_url = String::from("http://127.0.0.1:1");
        let (tx, rx) = mpsc::channel();
        state.receiver = Some(rx);
        state.busy = true;
        state.pending_probes = 1;
        state.pending_publish_after_probe = true;
        // When: probe volta Online.
        tx.send(MachinesMsg::Probed {
            node_url: String::from("http://127.0.0.1:1"),
            status: ProbeStatus {
                state: ProbeState::Online,
                detail: String::from("protocolo 1.0"),
            },
        })
        .expect("canal vivo");
        state.poll();
        // Then: publicação encadeada (busy rearmado pelo publish).
        assert!(state.busy);
        assert!(state.receiver.is_some());
        assert!(!state.pending_publish_after_probe);

        // Given: mesma pendência, outro estado.
        let mut state = MachinesState::with_url("http://127.0.0.1:1");
        state.form_machine_id = String::from("novo-70");
        state.form_node_url = String::from("http://127.0.0.1:1");
        let (tx, rx) = mpsc::channel();
        state.receiver = Some(rx);
        state.busy = true;
        state.pending_probes = 1;
        state.pending_publish_after_probe = true;
        // When: probe volta Offline.
        tx.send(MachinesMsg::Probed {
            node_url: String::from("http://127.0.0.1:1"),
            status: ProbeStatus {
                state: ProbeState::Offline,
                detail: String::from("fechado"),
            },
        })
        .expect("canal vivo");
        state.poll();
        // Then: cancelado com aviso, sem publicar.
        assert!(!state.busy);
        assert!(state.notice.contains("NÃO persistido"));
    }

    #[test]
    fn clear_form_empties_manual_fields() {
        // Given: formulário preenchido.
        let mut state = MachinesState::with_url("http://127.0.0.1:1");
        state.form_machine_id = String::from("x");
        state.form_node_url = String::from("http://x:4317");
        state.form_token = String::from("segredo");
        // When: Limpar.
        state.clear_form();
        // Then: os 3 campos do design vazios.
        assert!(state.form_machine_id.is_empty());
        assert!(state.form_node_url.is_empty());
        assert!(state.form_token.is_empty());
    }

    #[test]
    fn host_from_url_parses_authority() {
        assert_eq!(host_from_url("http://192.168.1.70:4317"), "192.168.1.70");
        assert_eq!(host_from_url("http://lab-01:4317/rpc"), "lab-01");
        assert_eq!(host_from_url("http://lab-01"), "lab-01");
        assert_eq!(host_from_url("http://user@h:4317/x?y=1"), "h");
        assert_eq!(host_from_url("http://[fe80::1]:4317"), "fe80::1");
    }

    #[test]
    fn auth_prefers_field_then_memory_token() {
        // Given: autoridade sem campo explícito, com token em memória.
        // (host `.invalid`: nunca tem token em disco — RFC 2606.)
        let mut state = MachinesState::with_url("http://no-existe.invalid:4317");
        state.tokens.insert(
            String::from("http://no-existe.invalid:4317"),
            String::from("mem"),
        );
        // Then: usa o da memória.
        assert_eq!(state.auth().as_deref(), Some("mem"));
        // When: campo explícito preenchido.
        state.authority_token = String::from("campo");
        // Then: o campo vence.
        assert_eq!(state.auth().as_deref(), Some("campo"));
        // When: ambos vazios.
        state.authority_token.clear();
        state.tokens.clear();
        // Then: sem auth.
        assert_eq!(state.auth(), None);
    }
}
