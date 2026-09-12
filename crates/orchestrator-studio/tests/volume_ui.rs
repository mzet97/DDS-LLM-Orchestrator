//! Volume e escala da apresentação (UI-6.3/UI-6.4, §22): 10 mil
//! linhas e 10 mil registros sintéticos, zoom 200% sem pânico.
//! Orçamento medido aqui é da APRESENTAÇÃO (render/filtro locais),
//! nunca do backend. Tempos impressos como evidência, sem alegação.

use std::time::Instant;

use orchestrator_studio::design::ResolvedTheme;
use orchestrator_studio::gallery::GalleryState;
use orchestrator_studio::operations::{OpState, OperationRecord, OperationsPanel};

#[test]
fn log_viewer_holds_10k_lines_with_stable_filter() {
    let mut viewer = orchestrator_studio::operations::LogViewer::new(10_000);
    let batch: Vec<String> = (0..10_000).map(|n| format!("linha {n:05}")).collect();
    let start = Instant::now();
    viewer.push_batch(&batch);
    viewer.query = String::from("00999");
    let visible = viewer.visible();
    let elapsed = start.elapsed();
    assert_eq!(viewer.len(), 10_000);
    assert_eq!(visible.len(), 1);
    eprintln!("log 10k + filtro: {elapsed:?}");
}

#[test]
fn operations_filter_scales_to_10k_records() {
    let mut panel = OperationsPanel::new();
    for n in 0..10_000 {
        panel.track(OperationRecord {
            id: format!("op-{n:05}"),
            kind: if n % 2 == 0 {
                String::from("tarefa de inferência")
            } else {
                String::from("implantação de servidor")
            },
            resource: format!("res-{n}"),
            machine: String::from("agents-01"),
            state: OpState::Done,
            events: Vec::new(),
            cancel_supported: false,
        });
    }
    let start = Instant::now();
    panel.kind_filter = String::from("inferência");
    let visible = panel.visible();
    let elapsed = start.elapsed();
    assert_eq!(visible.len(), 5_000);
    eprintln!("filtro 10k registros: {elapsed:?}");
}

#[test]
fn gallery_renders_at_200_percent_scale() {
    use egui_kittest::{kittest::Queryable as _, Harness};
    let mut harness = Harness::new_ui_state(
        |ui, state: &mut GalleryState| orchestrator_studio::gallery::show(ui, state),
        GalleryState::new(ResolvedTheme::Light),
    );
    harness.set_pixels_per_point(2.0);
    harness.run();
    harness.get_by_label("Galeria de componentes");
    harness.get_by_label("Ação primária");
}
