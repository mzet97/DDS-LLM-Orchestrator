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

use crate::catalog_remote::{
    delete_with_token, fetch_snapshot_with_token, publish_with_token, SharedCatalogError,
};
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
mod machines_actions;
mod machines_ops;
mod machines_state;

pub(crate) use machines_state::{
    host_from_url, non_empty, normalize_url, probe_status, MachinesMsg,
};
pub use machines_state::{MachinesError, MachinesState, ProbeState, ProbeStatus};

#[cfg(test)]
mod tests_catalog;
#[cfg(test)]
mod tests_state;
