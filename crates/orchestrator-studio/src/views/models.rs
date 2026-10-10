//! Painel 3.7 Modelos GGUF (inventário SHA-256): diretório + manifesto,
//! banner de progresso do worker real, 4 cards de resumo, filtros por status,
//! tabela calculado × manifesto com ações e rodapé do motor de checksum.
//!
//! Só dados reais: engine `sha2` (nada de BLAKE3/AVX-512/OpenSSL), hostname e
//! filesystem via libc, sem tópico DDS nem lease de verificador (o painel é
//! local — esses itens do mockup não existem e ficam FORA).

use crate::kit;
use crate::models::{ManifestStatus, ModelsState};
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, state: &mut ModelsState) {
    panel_header(
        ui,
        "SEC 3.7",
        "3.7 Modelos GGUF",
        "Inventário de tensores locais, validação e verificação de integridade criptográfica contra manifesto imutável.",
    );
    ui.horizontal(|ui| {
        kit::badge(ui, "MANIFESTO SHA-256", theme::PRIMARY_FIXED_DIM);
        kit::badge(
            ui,
            &format!("STORAGE-HOST: {}", hostname()),
            theme::ON_SURFACE_VARIANT,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Exportar Relatório").clicked() {
                state.export_report();
            }
            let verify = ui.add_enabled(
                !state.list.is_empty() && !state.is_busy(),
                egui::Button::new("↻ Verificar Tudo"),
            );
            if verify.clicked() {
                state.verify_all();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    state.poll();
    if state.is_busy() {
        ui.ctx().request_repaint();
    }

    // ── Diretório + manifesto (botões no topo: cliques kittest) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption(&format!(
                "DIRETÓRIO DE MODELOS · {}",
                fs_info(&state.dir).unwrap_or_else(|| String::from("—"))
            )));
            let mut dir = state.dir.display().to_string();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut dir)
                        .desired_width(300.0)
                        .hint_text("STUDIO_MODELS_DIR ou $HOME/tese/models"),
                )
                .changed()
            {
                state.dir = dir.into();
            }
        });
        ui.vertical(|ui| {
            ui.label(caption("CAMINHO DO MANIFESTO · SHA-256 IMUTÁVEL"));
            ui.add(
                egui::TextEdit::singleline(&mut state.manifest_path)
                    .desired_width(300.0)
                    .hint_text("benchmarks/orchestration/locks/models-manifest.json"),
            );
        });
        ui.vertical(|ui| {
            ui.label(caption("AÇÕES"));
            ui.horizontal(|ui| {
                if kit::primary_button(ui, "Inventariar").clicked() && !state.is_busy() {
                    state.refresh();
                }
                if ui.button("Carregar manifesto").clicked() {
                    state.load_manifest();
                }
            });
        });
    });
    if !state.notice.is_empty() {
        ui.label(egui::RichText::new(&state.notice).small().color(theme::OK));
    }
    if !state.error.is_empty() {
        kit::error_banner(ui, &state.error);
    }
    if state.dir.as_os_str().is_empty() {
        kit::empty_state(
            ui,
            "Nenhum diretório padrão: defina STUDIO_MODELS_DIR (ou edite o campo acima).",
        );
    }

    // ── Banner de progresso do worker real (bytes/taxa/ETA medidos) ──
    // Cancelar sai do empréstimo (E0500): marca aqui, executa depois.
    let mut cancel_clicked = false;
    if let Some(progress) = &state.hashing {
        let pct = progress.current_fraction() * 100.0;
        kit::accent_card(ui, theme::PRIMARY_CONTAINER, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "● CALCULANDO SHA-256 ({} de {} arquivos verificados)…",
                        progress.done, progress.total
                    ))
                    .strong()
                    .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new("× Cancelar").color(theme::ERROR))
                        .clicked()
                    {
                        cancel_clicked = true;
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Arquivo corrente: {} ({} / {pct:.0}% processado)",
                        progress.current,
                        fmt_gb2(progress.current_total),
                    ))
                    .small(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "IO: {:.1} MB/s · ETA: {}",
                            progress.rate_bps as f64 / (1024.0 * 1024.0),
                            progress
                                .current_eta_secs()
                                .map(fmt_hhmmss)
                                .unwrap_or_else(|| String::from("—")),
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                });
            });
            // Largura finita: INFINITY gera NaN no layout (assert egui).
            let bar_width = ui.available_width();
            ui.add(
                egui::ProgressBar::new(progress.current_fraction())
                    .desired_width(bar_width)
                    .fill(theme::PRIMARY_CONTAINER),
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Chunk: {} / {} (sha2 software, blocos de 64 KiB, 1 thread)",
                        fmt_gb2(progress.current_bytes),
                        fmt_gb2(progress.current_total),
                    ))
                    .small()
                    .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{pct:.1}%"))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                });
            });
        });
        ui.add_space(theme::SPACE_SM);
    }
    if cancel_clicked {
        state.cancel();
    }

    // Estado do manifesto (âncora literal: "manifesto carregado: N registro(s)").
    match (&state.manifest, state.manifest_error.is_empty()) {
        (Some(manifest), true) => {
            ui.label(
                egui::RichText::new(format!(
                    "manifesto carregado: {} registro(s)",
                    manifest.len()
                ))
                .monospace()
                .small()
                .color(theme::OK),
            );
        }
        (None, true) => {
            ui.label(
                egui::RichText::new("Sem manifesto carregado: os SHAs ficam sem cruzamento.")
                    .monospace()
                    .small()
                    .color(theme::STALE),
            );
        }
        _ => {
            kit::error_banner(ui, &state.manifest_error);
        }
    }
    ui.add_space(theme::SPACE_MD);

    // ── 4 cards de resumo (mockup 3.7) ──
    let total_bytes: u64 = state.list.iter().map(|item| item.size_bytes).sum();
    let ok_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Ok)
        .count();
    let deviants: Vec<&str> = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Desviado)
        .map(|item| item.file_name.as_str())
        .collect();
    let pending_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Pendente)
        .count();
    let unregistered_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::SemRegistro)
        .count();
    // Subtextos vinculados antes (temporário em `&format!` não sobrevive).
    // Valor do volume CURTO: texto longo quebra em 3 linhas no HERO 32px
    // (provado na captura) — o total do fs vai no subtexto.
    let fs_total = fs_total_bytes(&state.dir);
    let volume_value = format!("{:.2} GB", total_bytes as f64 / 1_000_000_000.0);
    let volume_sub = match fs_total {
        Some(total) => format!(
            "{} arquivo(s) .gguf · {:.1} GB no volume",
            state.list.len(),
            total as f64 / 1_000_000_000.0
        ),
        None => format!("{} arquivo(s) .gguf", state.list.len()),
    };
    let ok_sub = if state.list.is_empty() {
        String::from("nenhum arquivo")
    } else {
        format!(
            "{:.1}% da coleção conferida",
            ok_count as f64 * 100.0 / state.list.len() as f64
        )
    };
    let dev_value = if deviants.is_empty() {
        String::from("0")
    } else {
        format!("{} DESVIADO", deviants.len())
    };
    let dev_sub = if deviants.is_empty() {
        String::from("nenhuma divergência")
    } else {
        format!("{} · quarentena recomendada", deviants.join(", "))
    };
    let pipe_sub = if state.is_busy() {
        format!("EM VERIFICAÇÃO · {unregistered_count} sem manifesto · 1 thread")
    } else {
        String::from("ocioso")
    };
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Volume mapeado",
            volume_value,
            &volume_sub,
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Verificados OK",
            ok_count.to_string(),
            &ok_sub,
            theme::OK,
        );
        kit::metric_card(
            &mut cols[2],
            "Divergências (hash)",
            dev_value,
            &dev_sub,
            if deviants.is_empty() {
                theme::OK
            } else {
                theme::ERROR
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Pipeline de hash",
            if state.is_busy() {
                String::from("1")
            } else {
                String::from("0")
            },
            &pipe_sub,
            if state.is_busy() {
                theme::WARN
            } else {
                theme::STALE
            },
        );
    });
    ui.add_space(theme::SPACE_MD);

    if state.list.is_empty() && !state.is_busy() {
        kit::empty_state(
            ui,
            "Nenhum .gguf listado. Ajuste o diretório e clique em Inventariar.",
        );
        return;
    }

    // ── Artefatos + filtros por status (mockup 3.7) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Artefatos Armazenados")
                .strong()
                .color(theme::ON_SURFACE_VARIANT),
        );
        ui.label(
            egui::RichText::new(format!("{} Arquivos Mapeados", state.list.len()))
                .small()
                .weak(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (id, label) in [
                (2u8, format!("Pendentes ({pending_count})")),
                (1u8, format!("Divergentes ({})", deviants.len())),
                (0u8, format!("Todos ({})", state.list.len())),
            ] {
                if ui
                    .selectable_label(
                        state.filter == id,
                        egui::RichText::new(label).monospace().small(),
                    )
                    .clicked()
                {
                    state.filter = id;
                }
            }
            ui.label(
                egui::RichText::new("FILTRO:")
                    .small()
                    .strong()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── Tabela: checksum calculado × manifesto em colunas separadas ──
    kit::table("models-artifacts").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Nome do Arquivo",
                "Tamanho",
                "Checksum SHA-256 Calculado",
                "Checksum do Manifesto",
                "Status",
                "Ações",
            ],
        );
        for artifact in &state.list {
            let visible = match state.filter {
                1 => artifact.manifest_status == ManifestStatus::Desviado,
                2 => artifact.manifest_status == ManifestStatus::Pendente,
                _ => true,
            };
            if !visible {
                continue;
            }
            kit::mono_cell(ui, &artifact.file_name);
            kit::num_cell(
                ui,
                &format!("{:.1} MiB", artifact.size_bytes as f64 / 1_048_576.0),
            );
            if artifact.sha256_hex.is_empty() {
                match &state.hashing {
                    Some(progress) if progress.current == artifact.file_name => {
                        ui.label(
                            egui::RichText::new(format!(
                                "[Calculando… {:.0}%]",
                                progress.current_fraction() * 100.0
                            ))
                            .small()
                            .color(theme::WARN),
                        );
                    }
                    _ => {
                        ui.label(egui::RichText::new("pendente").small().color(theme::STALE));
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new(short_sha(&artifact.sha256_hex))
                        .monospace()
                        .small(),
                )
                .on_hover_text(&artifact.sha256_hex);
            }
            let expected = state
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.expected(&artifact.file_name));
            match expected {
                Some(sha) => {
                    ui.label(
                        egui::RichText::new(short_sha(sha))
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    )
                    .on_hover_text(sha);
                }
                None => {
                    ui.label(
                        egui::RichText::new("— sem registro")
                            .small()
                            .color(theme::STALE),
                    );
                }
            }
            // Status em TEXTO com rótulo EXATO (âncora kittest "OK": sem
            // prefixo ●, sem badge — badge some no accesskit).
            let status_color = match artifact.manifest_status {
                ManifestStatus::Ok => theme::OK,
                ManifestStatus::Desviado => theme::ERROR,
                ManifestStatus::Pendente | ManifestStatus::SemRegistro => theme::STALE,
            };
            ui.label(
                egui::RichText::new(artifact.manifest_status.label())
                    .monospace()
                    .color(status_color),
            );
            let copy = ui.add_enabled(
                !artifact.sha256_hex.is_empty(),
                egui::Button::new("Copiar SHA"),
            );
            if copy.clicked() {
                ui.ctx().copy_text(artifact.sha256_hex.clone());
            }
            ui.end_row();
        }
    });
    let hashed = state
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
    ui.label(
        egui::RichText::new(
            "Checksum Engine: sha2 software (RustCrypto) · blocos de 64 KiB · 1 thread",
        )
        .monospace()
        .small()
        .color(theme::ON_SURFACE_VARIANT),
    );
    ui.label(
        egui::RichText::new(format!(
            "{} arquivo(s), {} com SHA-256, total {:.1} GiB. Leitura apenas: nada é deletado.",
            state.list.len(),
            hashed,
            total_bytes as f64 / 1_073_741_824.0
        ))
        .small()
        .weak(),
    );
}

fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

/// SHA truncado para a tabela (32 hex + …; completo no hover).
fn short_sha(sha: &str) -> String {
    if sha.len() > 34 {
        format!("{}…", &sha[..32])
    } else {
        String::from(sha)
    }
}

/// GiB com 2 casas ("4.68 GB" do mockup usa GB decimal — aqui GB=1e9).
fn fmt_gb2(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
}

/// Segundos → HH:MM:SS (ETA do mockup).
fn fmt_hhmmss(secs: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Hostname da máquina (chip STORAGE-HOST); "—" se indisponível.
fn hostname() -> String {
    #[cfg(target_os = "linux")]
    {
        let mut buf = [0 as libc::c_char; 256];
        // SAFETY: gethostname escreve no máximo o tamanho dado; NUL garantido.
        let ok = unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len()) == 0 };
        if ok {
            // SAFETY: buf é NUL-terminado pelo gethostname em sucesso.
            let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
            if let Ok(name) = cstr.to_str() {
                if !name.is_empty() {
                    return String::from(name);
                }
            }
        }
        String::from("—")
    }
    #[cfg(not(target_os = "linux"))]
    {
        String::from("—")
    }
}

/// Filesystem do caminho ("ext4 · rw,noatime"); `None` se ilegível.
/// Tipo vem do `statfs` (magia), flags do `statvfs` (o `statfs` da libc
/// não expõe `f_flags` no x86_64).
fn fs_info(path: &std::path::Path) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: statfs preenche a struct zerada; caminho NUL válido.
        if unsafe { libc::statfs(cpath.as_ptr(), &mut stat) } != 0 {
            return None;
        }
        let fstype = match stat.f_type as u64 as u32 {
            0xEF53 => "ext4",
            0x01021994 => "tmpfs",
            0x9123683E => "btrfs",
            0x58465342 => "xfs",
            0x794C7630 => "overlayfs",
            0x6969 => "nfs",
            0xFF534D42 => "smb",
            0xFE534D42 => "smb2",
            0x65735546 => "fuse",
            _ => "fs",
        };
        let mut vfs: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: idem; flags de montagem (ST_RDONLY/ST_NOATIME).
        if unsafe { libc::statvfs(cpath.as_ptr(), &mut vfs) } != 0 {
            return Some(String::from(fstype));
        }
        let flags = vfs.f_flag;
        let rw = if flags & libc::ST_RDONLY != 0 {
            "ro"
        } else {
            "rw"
        };
        let noatime = if flags & libc::ST_NOATIME != 0 {
            ",noatime"
        } else {
            ""
        };
        Some(format!("{fstype} · {rw}{noatime}"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

/// Bytes totais do filesystem do caminho (`None` se ilegível).
fn fs_total_bytes(path: &std::path::Path) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: idem fs_info.
        if unsafe { libc::statfs(cpath.as_ptr(), &mut stat) } != 0 {
            return None;
        }
        let block = if stat.f_frsize > 0 {
            stat.f_frsize as u64
        } else {
            stat.f_bsize as u64
        };
        Some(stat.f_blocks as u64 * block)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatters_match_design() {
        assert_eq!(fmt_gb2(4_680_000_000), "4.68 GB");
        assert_eq!(fmt_gb2(0), "0.00 GB");
        assert_eq!(fmt_hhmmss(9), "00:00:09");
        assert_eq!(fmt_hhmmss(3723), "01:02:03");
        assert_eq!(short_sha("abcdef"), "abcdef");
        assert_eq!(
            short_sha("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            "0123456789abcdef0123456789abcdef…"
        );
    }

    #[test]
    fn host_and_fs_helpers_are_honest() {
        // Caminho inexistente nunca inventa filesystem.
        assert_eq!(
            fs_info(std::path::Path::new("/definitivamente/inexistente-3-7")),
            None
        );
        assert_eq!(
            fs_total_bytes(std::path::Path::new("/definitivamente/inexistente-3-7")),
            None
        );
        #[cfg(target_os = "linux")]
        {
            assert!(!hostname().is_empty(), "hostname real ou —");
            assert!(
                fs_total_bytes(std::path::Path::new("/tmp")).unwrap_or(0) > 0,
                "/tmp tem tamanho"
            );
        }
    }
}
