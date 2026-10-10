//! Painel 3.12 Workflow (pipeline A→B→C): formulário, métricas cumulativas,
//! três cards de estágio com conectores (estado por estágio), tabela de
//! auditoria e painel de exceção com o erro REAL do estágio que falhou.
//!
//! Com a feature `dds`, o corredor injetado em [`WorkflowState::start`]
//! replica o `run_seq` do `wf-run` (mesmos prompts congelados e montagem via
//! `client::wf_assembly`), dentro de um runtime tokio de thread (padrão da
//! GUI: a thread de UI nunca bloqueia). Sem `dds`, o painel informa como
//! habilitar.

#[cfg(feature = "dds")]
use crate::kit;
use crate::panel_header::panel_header;
#[cfg(feature = "dds")]
use crate::theme;
use crate::workflow::WorkflowState;
#[cfg(feature = "dds")]
use crate::workflow::{StageOut, WorkflowConfig, WorkflowEvent};
use eframe::egui;

/// Ponto de entrada do painel (feature `dds` — corredor DDS real).
#[cfg(feature = "dds")]
pub fn show(ui: &mut egui::Ui, state: &mut WorkflowState) {
    state.poll();
    // Faixa de contexto (espelho do PNG; só fatos do contrato: tópicos
    // Tasks→TaskOutput e QoS efetiva do `Tasks` em `dds_dataspace::qos` —
    // Reliable+TransientLocal KL50. FORA: SCHEMA TaskExecutionChain.idl
    // (não existe) e contagem RTPS de participantes (sem fonte na view).
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(
                "EXECUÇÃO DISTRIBUÍDA: §3.12 WORKFLOW PIPELINE A→B→C / TÓPICOS: \
                 Tasks → TaskOutput / QoS: RELIABLE_TRANSIENT_LOCAL · KL50",
            )
            .monospace()
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
    });
    ui.add_space(theme::SPACE_XS);
    panel_header(
        ui,
        "SEC 3.12 · WORKFLOW PIPELINE A→B→C · CADEIA DDS",
        "Workflow (A→B→C)",
        "Cadeia sequencial canônica pelo data space: cada estágio é uma task \
         DDS real reivindicada por agentes vivos · tópico Tasks · saída Tasks/TaskOutput",
    );

    let run_clicked = form(ui, state);
    if run_clicked && !state.busy && !state.config.entry.trim().is_empty() {
        let config = state.config.clone();
        state.start(move |id, tx| run_chain(id, config, tx));
    }
    metrics(ui, state);
    stage_cards(ui, state);
    audit_table(ui, state);
    exceptions(ui, state);
}

/// Sem DDS: orientação (a máquina de estado continua testável).
#[cfg(not(feature = "dds"))]
pub fn show(ui: &mut egui::Ui, state: &mut WorkflowState) {
    panel_header(
        ui,
        "SEC 3.12 · WORKFLOW PIPELINE A→B→C · CADEIA DDS",
        "Workflow (A→B→C)",
        "Cadeia sequencial canônica pelo data space",
    );
    ui.label(
        "Requer a feature `dds` na compilação do Studio (cargo build \
         --features dds) e agentes vivos no domínio informado.",
    );
    let _ = state;
}

#[cfg(feature = "dds")]
fn ui_available(state: &WorkflowState) -> bool {
    !state.busy && !state.config.entry.trim().is_empty()
}

/// Formulário em linha (espelho do PNG); devolve `true` quando o botão
/// Executar foi clicado. FORA (não existem): NODE REGISTRY, LEASE LIMIT,
/// ID/ENCODING do disparo e `Tasks/TaskTrigger` (o tópico é `Tasks`).
#[cfg(feature = "dds")]
fn form(ui: &mut egui::Ui, state: &mut WorkflowState) -> bool {
    section(ui, "CONFIGURAÇÃO DO PIPELINE DE ORQUESTRAÇÃO DDS");
    let mut run_clicked = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption("DOMÍNIO DDS"));
            ui.add(egui::DragValue::new(&mut state.config.domain).range(0..=u32::MAX));
        });
        ui.add_space(theme::SPACE_LG);
        ui.vertical(|ui| {
            ui.label(caption("MODELO ALVO (INFERÊNCIA)"));
            ui.add(egui::TextEdit::singleline(&mut state.config.model).desired_width(300.0));
        });
        ui.add_space(theme::SPACE_LG);
        ui.vertical(|ui| {
            ui.label(caption("TIMEOUT GLOBAL"));
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut state.config.timeout_ms)
                        .speed(1000)
                        .range(1..=u64::MAX),
                );
                ui.label(caption("MS"));
            });
        });
        // right_to_left: primeiro adicionado = mais à direita.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(ui_available(state), |ui| {
                let label = if state.busy {
                    "executando…"
                } else {
                    "▶ Executar Workflow DDS"
                };
                run_clicked = kit::primary_button(ui, label).clicked();
            });
            kit::badge(ui, "CHAIN 3-STAGES", theme::PRIMARY_FIXED_DIM);
        });
    });
    ui.add_space(theme::SPACE_XS);
    ui.label(caption(
        "PAYLOAD DE ENTRADA DO DISPARO / PROMPT INICIAL (TÓPICO: Tasks)",
    ));
    let entry_width = ui.available_width();
    ui.add(
        egui::TextEdit::multiline(&mut state.config.entry)
            .desired_rows(2)
            .desired_width(entry_width)
            .hint_text("texto da tarefa (vira ENTRADA: no prompt do estágio A)"),
    );
    run_clicked
}

/// Rótulo de campo do formulário (mono uppercase discreto, espelho do PNG).
#[cfg(feature = "dds")]
fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

#[cfg(feature = "dds")]
fn run_chain(workflow_id: u64, config: WorkflowConfig, tx: std::sync::mpsc::Sender<WorkflowEvent>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            let _ = tx.send(WorkflowEvent::Done {
                total_ms: 0,
                error: Some(format!("runtime tokio: {err}")),
            });
            return;
        }
    };
    let outcome = rt.block_on(chain(workflow_id, config, &tx));
    let _ = tx.send(outcome);
}

/// Réplica do `run_seq` do `wf-run` com os MESMOS prompts congelados.
#[cfg(feature = "dds")]
async fn chain(
    workflow_id: u64,
    config: WorkflowConfig,
    tx: &std::sync::mpsc::Sender<WorkflowEvent>,
) -> WorkflowEvent {
    use client::dds_impl::DdsClientDds;
    use client::wf_assembly::{self, ANALYSIS, REVIEW};
    use client::{ClientConfig, DdsClient};
    use std::time::Instant;

    let started = Instant::now();
    let config_client = ClientConfig {
        client_id: format!("studio-wf{workflow_id}"),
        dds_domain: config.domain,
        timeout_ms: config.timeout_ms,
    };
    let helper = DdsClient::new(config_client.clone());
    let client = match DdsClientDds::new(config_client) {
        Ok(client) => client,
        Err(err) => {
            return WorkflowEvent::Done {
                total_ms: started.elapsed().as_millis() as u64,
                error: Some(format!("cliente DDS: {err}")),
            };
        }
    };
    let mut prior: Vec<String> = Vec::new();
    let stages = [
        ("A", wf_assembly::SEQ_PROMPTS[0], vec![]),
        ("B", wf_assembly::SEQ_PROMPTS[1], vec![ANALYSIS]),
        ("C", wf_assembly::SEQ_PROMPTS[2], vec![ANALYSIS, REVIEW]),
    ];
    for (name, prompt, labels) in stages {
        let carried: Vec<(&str, String)> = labels
            .iter()
            .zip(prior.iter())
            .map(|(label, content)| (*label, content.clone()))
            .collect();
        let messages = wf_assembly::build_messages(prompt, &config.entry, &carried);
        let task = helper.create_task(&config.model, &messages, 5, false);
        match client.submit(task).await {
            Ok(result) if result.success => {
                let _ = tx.send(WorkflowEvent::Stage(StageOut {
                    stage: String::from(name),
                    task_id: result.task_id.clone(),
                    latency_ms: result.latency_ms,
                    preview: WorkflowState::preview(&result.content, 96),
                }));
                prior.push(result.content);
            }
            Ok(result) => {
                return WorkflowEvent::Done {
                    total_ms: started.elapsed().as_millis() as u64,
                    error: Some(format!(
                        "estágio {name} sem sucesso (task {})",
                        result.task_id
                    )),
                };
            }
            Err(err) => {
                return WorkflowEvent::Done {
                    total_ms: started.elapsed().as_millis() as u64,
                    error: Some(format!("estágio {name}: {err}")),
                };
            }
        }
    }
    WorkflowEvent::Done {
        total_ms: started.elapsed().as_millis() as u64,
        error: None,
    }
}

/// Estado visual de um estágio (derivação honesta do estado real).
#[cfg(feature = "dds")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum StageVisual {
    Pendente,
    Executando,
    Concluido,
    Falhou,
}

#[cfg(feature = "dds")]
fn stage_of<'a>(stages: &'a [StageOut], name: &str) -> Option<&'a StageOut> {
    stages.iter().find(|stage| stage.stage == name)
}

/// Nome canônico do estágio (espelho do PNG): a chave interna ("A"/"B"/"C"
/// do `wf_assembly`) ganha o rótulo do processo de negócio.
#[cfg(feature = "dds")]
fn stage_display(name: &str) -> String {
    match name {
        "A" => String::from("A Triagem & Fatos"),
        "B" => String::from("B Síntese & Resolução LLM"),
        "C" => String::from("C Validação & Formatação"),
        other => other.to_owned(),
    }
}

/// Inteiro com separador de milhar (espelho do PNG: `2,080 ms`).
#[cfg(any(test, feature = "dds"))]
fn fmt_int(n: u64) -> String {
    let digits = n.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

#[cfg(feature = "dds")]
fn stage_visual(state: &WorkflowState, name: &str) -> StageVisual {
    if stage_of(&state.stages, name).is_some() {
        return StageVisual::Concluido;
    }
    if let Some(error) = &state.error {
        if error.contains(&format!("estágio {name}")) {
            return StageVisual::Falhou;
        }
    }
    if state.busy {
        // Primeiro estágio sem card = o que está executando agora.
        let first_missing = ["A", "B", "C"]
            .iter()
            .find(|candidate| stage_of(&state.stages, candidate).is_none());
        if first_missing == Some(&name) {
            return StageVisual::Executando;
        }
    }
    StageVisual::Pendente
}

/// Faixa de 4 métricas do pipeline (espelho do PNG; 4º card honesto —
/// %LOSS/heartbeats/NACKs não têm fonte, então vale "Último erro").
#[cfg(feature = "dds")]
fn metrics(ui: &mut egui::Ui, state: &mut WorkflowState) {
    ui.add_space(theme::SPACE_MD);
    let lat = |name: &str| {
        stage_of(&state.stages, name)
            .map(|stage| fmt_int(stage.latency_ms))
            .unwrap_or_else(|| "—".to_owned())
    };
    let breakdown = format!(
        "Soma de Estágios A ({}) + B ({}) + C ({})",
        lat("A"),
        lat("B"),
        lat("C")
    );
    // Valores HERO 42px: curtos (achado 3.7 — longo quebra em 3 linhas).
    let (state_value, state_sub, state_accent) = if state.busy {
        ("Em execução", "estágios em andamento", theme::WARN)
    } else if state.error.is_some() {
        ("Falhou", "ver exceções abaixo", theme::ERROR)
    } else if state.total_ms.is_some() {
        ("Concluído", "sem retry silencioso", theme::OK)
    } else {
        ("Aguardando", "nenhuma execução ainda", theme::STALE)
    };
    let delivered = state.stages.len();
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Tempo total cumulativo",
            state
                .total_ms
                .map(|ms| format!("{} ms", fmt_int(ms)))
                .unwrap_or_else(|| "—".to_owned()),
            &breakdown,
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Estado do pipeline",
            state_value.to_owned(),
            state_sub,
            state_accent,
        );
        kit::metric_card(
            &mut cols[2],
            "Tópico de saída final",
            String::from("TaskOutput"),
            &format!("Tasks → TaskOutput · {delivered}/3 entregues"),
            if delivered == 3 {
                theme::OK
            } else if delivered > 0 {
                theme::WARN
            } else {
                theme::STALE
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Último erro",
            if state.error.is_some() {
                "sim".to_owned()
            } else {
                "nenhum".to_owned()
            },
            if state.busy {
                "executando…"
            } else {
                "ver exceções abaixo"
            },
            if state.error.is_some() {
                theme::ERROR
            } else {
                theme::OK
            },
        );
    });
    ui.add_space(theme::SPACE_MD);
}

/// Cadeia distribuída: 3 cards de estágio com conectores (espelho do PNG).
#[cfg(feature = "dds")]
fn stage_cards(ui: &mut egui::Ui, state: &mut WorkflowState) {
    section(ui, "CADEIA DISTRIBUÍDA EM EXECUÇÃO (A → B → C)");
    let visuals: [(String, StageVisual); 3] = [
        (String::from("A"), stage_visual(state, "A")),
        (String::from("B"), stage_visual(state, "B")),
        (String::from("C"), stage_visual(state, "C")),
    ];
    // Conectores A→B→C com o estágio de origem concluído destacado.
    let connector = |from: StageVisual| -> (egui::Color32, &'static str) {
        match from {
            StageVisual::Concluido => (theme::OK, "●——▶"),
            StageVisual::Falhou => (theme::ERROR, "●——×"),
            _ => (theme::STALE, "○——▶"),
        }
    };
    // `accent_card` força largura total: cada card vai num `vertical` de
    // largura fixa (sem isso só o A aparece — captura 3.12).
    let card_w = ((ui.available_width() - 2.0 * 56.0 - 4.0 * 8.0) / 3.0).max(230.0);
    ui.horizontal(|ui| {
        for (index, (name, visual)) in visuals.iter().enumerate() {
            if index > 0 {
                let (_, previous) = visuals[index - 1].clone();
                let (color, symbol) = connector(previous);
                ui.vertical(|ui| {
                    ui.set_min_width(56.0);
                    ui.set_max_width(56.0);
                    ui.add_space(48.0);
                    ui.label(
                        egui::RichText::new(symbol)
                            .monospace()
                            .color(color)
                            .size(16.0),
                    );
                });
            }
            let (accent, state_label) = match visual {
                StageVisual::Concluido => (theme::OK, "● CONCLUÍDO"),
                StageVisual::Executando => (theme::WARN, "○ EXECUTANDO"),
                StageVisual::Falhou => (theme::ERROR, "× FALHOU"),
                StageVisual::Pendente => (theme::STALE, "○ PENDENTE"),
            };
            ui.vertical(|ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                kit::accent_card(ui, accent, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(stage_display(name)).strong().size(14.0));
                        kit::badge(ui, state_label, accent);
                    });
                    match stage_of(&state.stages, name) {
                        Some(stage) => {
                            // task_id real é UUID v4 (36 chars): 8 à mostra,
                            // íntegra no hover. FORA: "Executor DDS" (o `TaskResult`
                            // não carrega executor) e latência "QoS SUB" (sem fonte).
                            ui.horizontal(|ui| {
                                ui.label(caption("TASK ID:"));
                                ui.label(
                                    egui::RichText::new(
                                        stage.task_id.chars().take(8).collect::<String>(),
                                    )
                                    .monospace()
                                    .small()
                                    .color(theme::PRIMARY_FIXED_DIM),
                                )
                                .on_hover_text(&stage.task_id);
                            });
                            ui.horizontal(|ui| {
                                ui.label(caption("LATÊNCIA:"));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} ms",
                                        fmt_int(stage.latency_ms)
                                    ))
                                    .monospace()
                                    .small()
                                    .color(theme::PRIMARY_FIXED_DIM),
                                );
                            });
                            ui.label(caption("PRÉVIA DO BUFFER (96 CHARS):"));
                            egui::Frame::NONE
                                .fill(theme::SURFACE_LOW)
                                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
                                .inner_margin(theme::SPACE_SM)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.label(
                                        egui::RichText::new(if stage.preview.is_empty() {
                                            "—"
                                        } else {
                                            &stage.preview
                                        })
                                        .small()
                                        .weak(),
                                    );
                                });
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("DDS PUB: Tasks")
                                        .monospace()
                                        .small()
                                        .color(theme::ON_SURFACE_VARIANT),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new("OK 100%")
                                                .monospace()
                                                .small()
                                                .strong()
                                                .color(theme::PRIMARY_FIXED_DIM),
                                        );
                                    },
                                );
                            });
                        }
                        None => {
                            ui.label(
                                egui::RichText::new("aguardando o estágio anterior…")
                                    .small()
                                    .weak(),
                            );
                        }
                    }
                });
            });
        }
    });
    ui.add_space(theme::SPACE_MD);
}

/// Tabela de auditoria dos frames entregues (mesma fonte dos cards).
/// FORA: AGENTE EXECUTOR (sem executor no `TaskResult`) e TIMESTAMP RTPS
/// (sem carimbo por estágio no fio).
#[cfg(feature = "dds")]
fn audit_table(ui: &mut egui::Ui, state: &mut WorkflowState) {
    if state.stages.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(header(&format!(
            "AUDITORIA DE FRAMES DDS NO DOMÍNIO {}",
            state.config.domain
        )));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} / 3 DELIVERED", state.stages.len()))
                    .monospace()
                    .small()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    kit::table("workflow_stages").show(ui, |ui| {
        kit::grid_header(
            ui,
            &["Estágio", "Task ID", "Latência", "Buffer (prévia 96 chars)"],
        );
        for stage in &state.stages {
            ui.label(egui::RichText::new(stage_display(&stage.stage)).strong());
            ui.label(
                egui::RichText::new(stage.task_id.chars().take(8).collect::<String>())
                    .monospace()
                    .color(theme::ON_SURFACE_VARIANT),
            )
            .on_hover_text(&stage.task_id);
            kit::num_cell(ui, &format!("{} ms", fmt_int(stage.latency_ms)));
            ui.label(egui::RichText::new(&stage.preview).small().weak());
            ui.end_row();
        }
    });
    if let Some(total_ms) = state.total_ms {
        if state.error.is_none() {
            ui.label(
                egui::RichText::new(format!(
                    "total: {} ms (3 estágios encadeados)",
                    fmt_int(total_ms)
                ))
                .monospace()
                .small()
                .color(theme::OK),
            );
        }
    }
    ui.add_space(theme::SPACE_MD);
}

/// Título de seção para fileiras (o `section_label` do kit consome a linha
/// toda e quebra `horizontal` — ver 3.6).
#[cfg(feature = "dds")]
fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Seção com hairline (os títulos saíram centralizados com o
/// `section_label` do kit na captura 3.12; aqui à esquerda, espelho do PNG).
#[cfg(feature = "dds")]
fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(header(text));
    ui.separator();
    ui.add_space(theme::SPACE_SM);
}

/// Tratamento de exceções: o erro REAL do estágio que falhou. FORA:
/// "SIMULAÇÃO DE FALHA" (não existe simulação), caixas nó/agente/postura e
/// trace RTPS (sem fonte no fio).
#[cfg(feature = "dds")]
fn exceptions(ui: &mut egui::Ui, state: &mut WorkflowState) {
    section(ui, "TRATAMENTO DE EXCEÇÕES & TIMEOUT POR ESTÁGIO (§3.12)");
    if let Some(error) = &state.error {
        kit::error_banner(
            ui,
            &format!(
                "{error} — o pipeline para no primeiro estágio que falha (sem retry silencioso)"
            ),
        );
    } else if state.busy {
        ui.label(
            egui::RichText::new("executando — falhas aparecem aqui com o motivo real")
                .small()
                .weak(),
        );
    } else {
        ui.label(
            egui::RichText::new("nenhuma exceção na última execução")
                .small()
                .color(theme::OK),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_int;

    #[test]
    fn fmt_int_groups_thousands() {
        assert_eq!(fmt_int(0), "0");
        assert_eq!(fmt_int(98), "98");
        assert_eq!(fmt_int(1_840), "1,840");
        assert_eq!(fmt_int(2_080), "2,080");
        assert_eq!(fmt_int(1_000_000), "1,000,000");
    }
}
