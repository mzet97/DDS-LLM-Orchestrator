//! Modo protegido pela UI (T-890-08, G-38/65): desarmado RECUSA sem
//! trafegar; armado exige "Confirmar" antes de tocar a máquina remota.

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::protected::ProtectedGuard;
use orchestrator_studio::views;

/// Painel de Serviços com URL inalcançável: qualquer actuação disparada
/// produz erro de rede VISÍVEL — prova de que a chamada FOI feita.
#[test]
fn disarmed_guard_refuses_actuation_without_touching_the_node() {
    let mut panel = orchestrator_studio::services::ServicesPanel::with_url(String::from(
        "http://127.0.0.1:1", // nada escuta aqui
    ));
    let mut guard = ProtectedGuard::new(); // DESARMADO
                                           // Fase pura (antes do harness): a RECUSA acontece no nível do guard —
                                           // a mesma API que a view usa. Desarmado, NADA é aceito para execução.
    match guard.request("PARAR serviço 'dds-agent' no nó") {
        orchestrator_studio::protected::ProtectedOutcome::Refused(refusal) => {
            assert!(refusal.contains("NÃO executada"));
        }
        _ => panic!("desarmado deve recusar"),
    }

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (panel, guard)| views::services::show(ui, panel, guard),
        (&mut panel, &mut guard),
    );
    harness.set_size(egui::vec2(900.0, 500.0));
    harness.run_steps(5);
    let (panel, guard) = harness.state();
    assert!(panel.pending_action.is_none(), "nada aceito para execução");
    assert!(
        guard
            .last_refusal
            .as_deref()
            .is_some_and(|r| r.contains("NÃO executada")),
        "recusa registrada para exibição"
    );
}

#[test]
fn armed_guard_requires_confirmation_before_actuation() {
    let mut panel =
        orchestrator_studio::services::ServicesPanel::with_url(String::from("http://127.0.0.1:1"));
    let mut guard = ProtectedGuard::new();
    guard.set_armed(true);

    // Armado: o pedido fica PENDENTE — a UI mostra Confirmar/Cancelar e o
    // painel só executa após "✔ Confirmar".
    match guard.request("PARAR serviço 'dds-agent' no nó") {
        orchestrator_studio::protected::ProtectedOutcome::NeedsConfirmation(_) => {}
        _ => panic!("armado deve pedir confirmação"),
    }
    panel.pending_action = Some((String::from("dds-agent"), false));

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (panel, guard)| views::services::show(ui, panel, guard),
        (&mut panel, &mut guard),
    );
    harness.set_size(egui::vec2(900.0, 500.0));
    harness.run_steps(5);

    // Expande o collapsing do painel (conteúdo só entra na árvore aberto).
    harness
        .get_by_label("Serviços do nó (plano: pretendido × efetivo)")
        .click();
    harness.run_steps(5);

    harness.get_by_label_contains("CONFIRMAR: PARAR serviço 'dds-agent' no nó");
    harness.get_by_label("✔ Confirmar").click();
    harness.run_steps(5);

    // Confirmado: o painel DISPAROU a actuação (erro de rede visível —
    // nada escuta em 127.0.0.1:1) e a pendência foi consumida.
    let (panel, guard) = harness.state();
    assert!(
        panel.pending_action.is_none(),
        "confirmação consome payload"
    );
    assert!(
        !panel.error.is_empty(),
        "actuação disparada (erro de conexão esperado com URL inalcançável): {}",
        panel.error
    );
    assert!(guard.pending.is_none());
}
