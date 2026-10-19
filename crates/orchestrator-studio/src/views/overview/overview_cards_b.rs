//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

/// Cartão 4 — Modelos GGUF (inventário × manifesto SHA-256).
pub(super) fn models_card(models: &ModelsState, dot: egui::Color32) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    let ok = models
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Ok)
        .count();
    let deviated = models
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Desviado)
        .count();
    let pending_name = models
        .list
        .iter()
        .find(|item| item.manifest_status == ManifestStatus::Pendente)
        .map(|item| item.file_name.chars().take(20).collect());
    let volume_gb = models.list.iter().map(|item| item.size_bytes).sum::<u64>() as f64 / 1e9;
    TeleCard {
        dot,
        title: String::from("Modelos GGUF"),
        status: if models.list.is_empty() {
            String::from("Vazio")
        } else {
            format!("{} no Inventário", models.list.len())
        },
        status_color: if models.list.is_empty() {
            theme::STALE
        } else {
            theme::ON_SURFACE_VARIANT
        },
        subtitle: String::from("Repositório de Pesos & Manifestos SHA256"),
        rows: vec![
            (
                String::from("Conformidade SHA:"),
                if models.list.is_empty() {
                    String::from("—")
                } else {
                    format!("{ok} OK · {deviated} DESVIADO")
                },
                if deviated > 0 {
                    theme::WARN
                } else if models.list.is_empty() {
                    theme::STALE
                } else {
                    cyan
                },
            ),
            (
                String::from("Checksum Pendente:"),
                pending_name.unwrap_or_else(|| String::from("—")),
                cyan,
            ),
            (
                String::from("Manifesto:"),
                if models.manifest_path.is_empty() {
                    String::from("nenhum")
                } else {
                    models
                        .manifest_path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&models.manifest_path)
                        .chars()
                        .take(20)
                        .collect()
                },
                cyan,
            ),
        ],
        footer: (
            String::from("Volume Total:"),
            format!("{volume_gb:.1} GB mapeados"),
        ),
    }
}

/// Cartão 5 — Inferência via presença DDS (ServerStatus) + prova local.
pub(super) fn inference_card(
    discovery: &DiscoveryState,
    proof: &str,
    dot: egui::Color32,
) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    match discovery.servers.first() {
        Some(server) => TeleCard {
            dot,
            title: String::from("Inferência (Llama-Server)"),
            status: String::from("Presença DDS Ok"),
            status_color: theme::OK,
            subtitle: String::from("Engine de Inferência Local"),
            rows: vec![
                (
                    String::from("Modelo Carregado:"),
                    server.model_loaded.chars().take(20).collect(),
                    cyan,
                ),
                (
                    String::from("Slots de Contexto:"),
                    format!(
                        "{}/{} ({} livres)",
                        server.slots_processing,
                        server.slots_idle + server.slots_processing,
                        server.slots_idle
                    ),
                    cyan,
                ),
                (
                    String::from("API Compatível:"),
                    String::from("OpenAI /v1/chat/completions"),
                    cyan,
                ),
            ],
            footer: (
                String::from("Prova local:"),
                if proof.is_empty() {
                    String::from("—")
                } else {
                    proof.chars().take(28).collect()
                },
            ),
        },
        None => TeleCard {
            dot,
            title: String::from("Inferência (Llama-Server)"),
            status: String::from("Sem presença"),
            status_color: theme::STALE,
            subtitle: String::from("Engine de Inferência Local"),
            rows: vec![
                (
                    String::from("Modelo Carregado:"),
                    String::from("—"),
                    theme::STALE,
                ),
                (
                    String::from("Slots de Contexto:"),
                    String::from("—"),
                    theme::STALE,
                ),
                (
                    String::from("API Compatível:"),
                    String::from("OpenAI /v1/chat/completions"),
                    cyan,
                ),
            ],
            footer: (
                String::from("Prova local:"),
                if proof.is_empty() {
                    String::from("—")
                } else {
                    proof.chars().take(28).collect()
                },
            ),
        },
    }
}

/// Cartão 6 — Catálogo compartilhado (snapshot + cursor + subdivisão real).
pub(super) fn catalog_card(shared: &SharedCatalog, summary: &str, dot: egui::Color32) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    let items = shared
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.items.as_slice())
        .unwrap_or(&[]);
    // Subdivisão honesta: conta por primeiro segmento do id ("machine/…").
    let mut groups: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for item in items {
        let segment = item.id.0.split('/').next().unwrap_or("?");
        *groups.entry(segment).or_default() += 1;
    }
    let mut ranked: Vec<(&str, usize)> = groups.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let subdivision = if ranked.is_empty() {
        String::from("—")
    } else {
        ranked
            .iter()
            .take(2)
            .map(|(segment, count)| format!("{segment} ({count})"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    TeleCard {
        dot,
        title: String::from("Catálogo Compartilhado"),
        status: shared
            .snapshot
            .as_ref()
            .map(|snapshot| format!("Snapshot cursor {}", snapshot.cursor.0))
            .unwrap_or_else(|| String::from("Aguardando")),
        status_color: if shared.snapshot.is_some() {
            theme::ON_SURFACE_VARIANT
        } else {
            theme::STALE
        },
        subtitle: String::from("Registro Central Distribuído de Estado"),
        rows: vec![
            (
                String::from("Total Registros:"),
                format!("{} itens mapeados", items.len()),
                cyan,
            ),
            (String::from("Subdivisão:"), subdivision, cyan),
            (
                String::from("Autoridade:"),
                if shared.url.is_empty() {
                    String::from("—")
                } else {
                    kit::authority(&shared.url)
                },
                cyan,
            ),
        ],
        footer: (String::from("Snapshot:"), summary.to_owned()),
    }
}
