//! Parte de `catalog_remote` (extraída para manter os arquivos revisáveis).

use super::*;

/// Resultado do acompanhamento de eventos (T-830-02), resolvido no worker.
pub(crate) enum FollowOutcome {
    /// Sem eventos: o cursor local já cobre o log.
    UpToDate,
    /// Só exclusões: o evento descreve tudo (id + revisão); a remoção é
    /// aplicada incrementalmente na tabela exibida.
    Deleted(Vec<Event>),
    /// Criação/atualização presente: o evento NÃO carrega o valor, então o
    /// lote termina em snapshot fresco (convergência honesta da tabela).
    /// Os eventos viajam para o feed (seq+id+rev de cada um).
    Applied {
        events: Vec<Event>,
        snapshot: Result<Snapshot, String>,
    },
    /// 410 `CursorExpired`: ressincronização total pelo snapshot.
    Resync(Result<Snapshot, String>),
    /// Falha de transporte: estado permanece; motivo vira aviso.
    Failed(String),
}

/// Mensagem do worker do catálogo.
pub(crate) enum CatalogMsg {
    /// Resposta de leitura do snapshot.
    Snapshot(Result<Snapshot, String>),
    /// Resposta de mutação + releitura do snapshot (mesma sequência do
    /// antigo `publish_form`/`delete_form`: muta, depois relê). O trio
    /// (id, base, result) alimenta o conflito estruturado e o feed — o
    /// `notice` em texto segue intacto (contrato com os testes de fio).
    Mutated {
        notice: String,
        then_snapshot: Option<Result<Snapshot, String>>,
        id: String,
        base: Option<u64>,
        is_delete: bool,
        result: Result<u64, SharedCatalogError>,
    },
    /// Resultado do acompanhamento de eventos (T-830-02).
    Followed(FollowOutcome),
}

/// Etiqueta honesta de uma entrada do feed (tela 3.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedTag {
    /// Publicação aplicada (nossa ou acompanhada).
    Committed,
    /// Tombstone aplicado (nosso ou acompanhado).
    Tombstone,
    /// Mutação rejeitada com 409 (nada aplicado).
    Rejected,
    /// Snapshot (re)servido pela autoridade.
    Snapshot,
}

impl FeedTag {
    /// Rótulo do mockup 3.9.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            FeedTag::Committed => "REV_COMMITTED",
            FeedTag::Tombstone => "TOMBSTONE",
            FeedTag::Rejected => "409_OCC-REJECT",
            FeedTag::Snapshot => "SNAPSHOT",
        }
    }
}

/// Entrada do feed com relógio (tela 3.9; mais nova no fim).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedEntry {
    pub ts_ms: u64,
    pub tag: FeedTag,
    pub text: String,
}

/// Conflito OCC estruturado (banner 409 de 1ª classe, tela 3.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictInfo {
    /// Base que o formulário tentou (None = criação).
    pub attempted_base: Option<u64>,
    /// Vigente devolvida pelo nó (None = sem vigente).
    pub current: Option<u64>,
}
