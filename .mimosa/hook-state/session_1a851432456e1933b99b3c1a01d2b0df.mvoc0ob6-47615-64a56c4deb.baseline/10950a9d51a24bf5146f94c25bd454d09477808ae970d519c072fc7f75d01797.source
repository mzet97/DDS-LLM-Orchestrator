//! 3.14 Logs: fluxo completo num ÚNICO teste sequencial — o ring buffer
//! é global por processo e testes paralelos no mesmo target competiriam.
//! Popula pela API pública (`info_at`/`warn_at`/`error_at`): ponta a ponta
//! sem mocks (push → buffer → view → export).

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::studio_log;
use orchestrator_studio::views;

#[test]
fn logs_panel_flow() {
    studio_log::clear();
    studio_log::set_mirror(true);
    let mut panel = studio_log::LogsPanel::new();
    let mut harness =
        egui_kittest::Harness::new_ui_state(|ui, panel| views::logs::show(ui, panel), &mut panel);
    harness.set_size(egui::vec2(1600.0, 2600.0));
    harness.run_steps(5);

    // 1. Vazio honesto.
    harness.get_by_label_contains("0/500");
    harness.get_by_label_contains("Falhas Ativas");
    harness.get_by_label("Todos (0)");
    harness.get_by_label_contains("(nenhum evento no filtro atual)");
    harness.get_by_label_contains("STREAM: REALTIME ATIVO");

    // 2. Popula 5 (sleep separa os ms: só o topo tem offset +0ms).
    studio_log::info_at("DISCOVERY", None, String::from("klog14 varredura ok"));
    std::thread::sleep(std::time::Duration::from_millis(2));
    studio_log::info_at("CATALOG", None, String::from("klog14 snapshot ok"));
    std::thread::sleep(std::time::Duration::from_millis(2));
    studio_log::info_at("GUI", None, String::from("klog14 carga ok"));
    std::thread::sleep(std::time::Duration::from_millis(2));
    studio_log::warn_at("HTTP", None, String::from("klog14-warn retry 401"));
    std::thread::sleep(std::time::Duration::from_millis(2));
    studio_log::error_at(
        "SYSTEMD",
        Some(String::from(r#"{"unit":"llama-server"}"#)),
        String::from("klog14-erro falha unit"),
    );
    harness.run_steps(3);

    harness.get_by_label_contains("5/500");
    harness.get_by_label_contains("Falhas Ativas");
    harness.get_by_label_contains("20.0% ERRO");
    harness.get_by_label_contains("1% cheio");
    harness.get_by_label_contains("0 descartados");
    harness.get_by_label("Todos (5)");
    harness.get_by_label("ERRO (1)");
    harness.get_by_label("WARN (1)");
    harness.get_by_label_contains("TAXA DE ABSORÇÃO");
    harness.get_by_label_contains("RETENÇÃO:");
    harness.get_by_label_contains("varredura ok");
    harness.get_by_label("[DISCOVERY]");
    harness.get_by_label("+0ms");

    // 3. Filtro ERRO: só a falha permanece.
    harness.get_by_label("ERRO (1)").click();
    harness.run_steps(3);
    harness.get_by_label_contains("falha unit");
    let gone = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        harness.get_by_label_contains("varredura ok");
    }));
    assert!(gone.is_err(), "info some sob filtro ERRO");
    harness.get_by_label("Todos (5)").click();
    harness.run_steps(3);
    harness.get_by_label_contains("varredura ok");

    // 4. Busca substring (estado direto — a digitação é do egui).
    harness.state_mut().search = String::from("klog14-warn");
    harness.run_steps(3);
    harness.get_by_label_contains("retry 401");
    let gone = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        harness.get_by_label_contains("varredura ok");
    }));
    assert!(gone.is_err(), "fora da busca some");
    harness.state_mut().search.clear();
    harness.run_steps(3);

    // 5. Seleção abre o inspetor forense (track_caller atravessa).
    let slot = studio_log::entries()
        .iter()
        .find(|entry| entry.message.contains("falha unit"))
        .expect("erro registrado")
        .slot;
    harness.get_by_label(&slot.to_string()).click();
    harness.run_steps(3);
    harness.get_by_label_contains(&format!("INSPETOR DIAGNÓSTICO · SLOT #{slot}"));
    harness.get_by_label_contains("ThreadId(");
    harness.get_by_label_contains("kittest_logs.rs");
    harness.get_by_label("RESUMO DA MENSAGEM RAW");
    harness.get_by_label("CONTEXTO SERIALIZADO DO NÓ (JSON)");
    harness.get_by_label("CALL STACK TRACE (RUST · CAPTURADO NO REGISTRO)");
    assert_eq!(
        harness
            .get_all_by_value(r#"{"unit":"llama-server"}"#)
            .count(),
        2,
        "payload no multiline (pai + TextRun)"
    );
    harness.get_by_label("COPIAR DUMP");

    // 6. Espelho stderr alterna o flag global.
    harness.get_by_label("Espelhar stderr").click();
    harness.run_steps(3);
    assert!(!studio_log::mirror(), "desligou");
    harness.get_by_label("Espelhar stderr").click();
    harness.run_steps(3);
    assert!(studio_log::mirror(), "religou");

    // 7. Exportar escreve o .log (e registra o ato: 6ª entrada).
    harness.get_by_label("Exportar (.log)").click();
    harness.run_steps(3);
    let content = std::fs::read_to_string(std::env::temp_dir().join("studio-logs.log"))
        .expect("relatório existe");
    assert!(
        content.contains("klog14-erro falha unit"),
        "marcador no arquivo"
    );
    harness.get_by_label("Todos (6)");

    // 8. Pausa congela a foto; o buffer segue absorvendo.
    harness.get_by_label("Pausar").click();
    harness.run_steps(3);
    harness.get_by_label("Retomar");
    harness.get_by_label_contains("STREAM: PAUSADO (foto)");
    studio_log::info_at("GUI", None, String::from("klog14 durante pausa"));
    harness.run_steps(3);
    harness.get_by_label("Todos (6)");
    harness.get_by_label("Retomar").click();
    harness.run_steps(3);
    harness.get_by_label("Todos (7)");

    // 9. Limpar esvazia tudo.
    harness.get_by_label("Limpar buffer").click();
    harness.run_steps(3);
    harness.get_by_label("Todos (0)");
    harness.get_by_label_contains("(nenhum evento no filtro atual)");
    studio_log::set_mirror(true);
}
