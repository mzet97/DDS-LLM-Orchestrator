//! Painel 3.13 Ferramentas & Tool Calls (T-890-06, G-18..22): governança ao
//! vivo do `ToolCall.Request` — cards de nível de segurança no vocabulário
//! do PRD (N0 READ_ONLY / N1 SANDBOX_EXEC / N2 HOST_MUTATION), chips de
//! filtro por status canônico com contagem + taxa REQ/s + bloqueios da
//! sessão, tabela com requester/nível/duração e inspetor REQUEST×RESPONSE
//! lado a lado (payloads íntegros do fio). Usa o MESMO `DdsState` da
//! Topologia.

use crate::dds_observe::{status_label, DdsState};
use crate::kit;
use crate::panel_header::panel_header;
use crate::protected::ProtectedGuard;
use crate::theme;
use eframe::egui;

/// Status canônicos do contrato para os chips de filtro (ordem do PNG).
const STATUS_FILTERS: [(i32, &str); 6] = [
    (0, "PENDING"),
    (1, "ALLOWED"),
    (3, "EXECUTING"),
    (4, "COMPLETED"),
    (2, "DENIED"),
    (5, "FAILED"),
];

pub fn show(ui: &mut egui::Ui, dds: &mut DdsState, guard: &ProtectedGuard) {
    dds.poll();
    // QoS efetiva do `ToolCall.Request` (`dds-dataspace/src/qos.rs:275`):
    // Reliable + TransientLocal KL10 — o selo do PNG é real. FORA: tópico
    // `ToolCall/Response` (o resultado volta na MESMA instância) e "CDR
    // Serializer" (sem fonte).
    panel_header(
        ui,
        &format!(
            "SEC 3.13 · DOMÍNIO {} · DDS-QoS: RELIABLE / TRANSIENT_LOCAL · REQ: ToolCall.Request",
            dds.domain
        ),
        "Ferramentas & Tool Calls",
        "Chamadas observadas no tópico ToolCall.Request: o gateway reivindica, \
         o policy-engine decide pelo nível e o resultado volta na MESMA \
         instância (at-least-once, sem tópico de resposta).",
    );

    show_controls(ui, dds, guard);
    if !show_cards(ui, dds) {
        return;
    }
    show_chips(ui, dds);
    show_table(ui, dds);
    show_inspector(ui, dds);
}

mod tools_cards;
mod tools_chips;
mod tools_controls;
mod tools_helpers;
mod tools_inspector;
mod tools_table;

use tools_cards::show_cards;
use tools_chips::show_chips;
use tools_controls::show_controls;
use tools_helpers::{fmt_int, header, section, short_call_id};
use tools_inspector::show_inspector;
use tools_table::show_table;

#[cfg(test)]
mod tests;
