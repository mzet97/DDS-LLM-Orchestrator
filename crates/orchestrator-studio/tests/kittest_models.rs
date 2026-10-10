//! 3.7 Modelos: filtros, exportação com leitura do JSON, re-verificação
//! após corrupção e cancelamento intra-arquivo — tudo contra arquivos reais
//! em diretório temporário (hash `sha2` de verdade, sem mocks).

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::views;

fn temp_models_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "studio-kittest-models-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir temporário");
    dir
}

fn sha_of(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Fluxo com 2 arquivos (1 OK + 1 desviado): cards, filtros, exportar com
/// leitura do relatório.
#[test]
fn models_filters_cards_and_export_report() {
    let dir = temp_models_dir("filters");
    std::fs::write(dir.join("a_ok.gguf"), b"conteudo ok").expect("escreve ok");
    std::fs::write(dir.join("b_bad.gguf"), b"conteudo adulterado").expect("escreve ruim");
    let manifest_path = dir.join("models-manifest.json");
    std::fs::write(
        &manifest_path,
        format!(
            "{{\"models\": {{\"a_ok.gguf\": \"{}\", \"b_bad.gguf\": \"{}\" }}}}",
            sha_of(b"conteudo ok"),
            "0".repeat(64),
        ),
    )
    .expect("escreve manifesto");

    let mut state = ModelsState::new();
    state.dir = dir.clone();
    state.manifest_path = manifest_path.display().to_string();

    let mut harness = egui_kittest::Harness::new_ui_state(views::models::show, state);
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label("Inventariar").click_accesskit();
    harness.run_steps(30);
    harness.get_by_label("Carregar manifesto").click_accesskit();
    harness.run_steps(10);

    harness.get_by_label("manifesto carregado: 2 registro(s)");
    harness.get_by_label("OK");
    harness.get_by_label("DESVIADO");
    harness.get_by_label_contains("50.0% da coleção conferida");
    harness.get_by_label("1 DESVIADO");
    harness.get_by_label_contains("quarentena recomendada");
    harness.get_by_label_contains("2 arquivo(s) .gguf");
    harness.get_by_label_contains("Checksum Engine: sha2 software");

    // Filtro Divergentes isola o b_bad; Todos restaura.
    harness.get_by_label("Divergentes (1)").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label("b_bad.gguf");
    harness.get_by_label("Todos (2)").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label("a_ok.gguf");

    // Exportar escreve JSON real com os 2 artefatos e status.
    harness.get_by_label("Exportar Relatório").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("relatório exportado:");
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("studio-models-report.json")).expect("relatório existe"),
    )
    .expect("relatório é JSON");
    let files: Vec<&str> = report
        .as_array()
        .expect("relatório é lista")
        .iter()
        .filter_map(|entry| entry["file"].as_str())
        .collect();
    assert!(files.contains(&"a_ok.gguf") && files.contains(&"b_bad.gguf"));

    std::fs::remove_dir_all(&dir).ok();
}

/// "Verificar Tudo" re-hasha a lista atual: arquivo corrompido depois do
/// OK vira DESVIADO (2 divergentes).
#[test]
fn models_verify_all_detects_later_corruption() {
    let dir = temp_models_dir("reverify");
    std::fs::write(dir.join("a_ok.gguf"), b"bytes originais").expect("escreve");
    let manifest_path = dir.join("models-manifest.json");
    std::fs::write(
        &manifest_path,
        format!(
            "{{\"models\": {{\"a_ok.gguf\": \"{}\" }}}}",
            sha_of(b"bytes originais"),
        ),
    )
    .expect("escreve manifesto");

    let mut state = ModelsState::new();
    state.dir = dir.clone();
    state.manifest_path = manifest_path.display().to_string();

    let mut harness = egui_kittest::Harness::new_ui_state(views::models::show, state);
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label("Inventariar").click_accesskit();
    harness.run_steps(30);
    harness.get_by_label("Carregar manifesto").click_accesskit();
    harness.run_steps(10);
    harness.get_by_label("OK");

    // Corrompe no disco e re-verifica: vira DESVIADO.
    std::fs::write(dir.join("a_ok.gguf"), b"bytes ADULTERADOS").expect("corrompe");
    harness.get_by_label("↻ Verificar Tudo").click_accesskit();
    harness.run_steps(30);
    harness.get_by_label("DESVIADO");
    harness.get_by_label("1 DESVIADO");

    std::fs::remove_dir_all(&dir).ok();
}

/// Cancelar durante hash de arquivo grande: banner honesto, sem travar.
#[test]
fn models_cancel_during_big_hash() {
    let dir = temp_models_dir("cancel");
    // 300 MB de zeros: hash leva ~1 s; o teste age em ms (margem 20×).
    std::fs::write(dir.join("big.gguf"), vec![0u8; 300 * 1024 * 1024]).expect("escreve grande");

    let mut state = ModelsState::new();
    state.dir = dir.clone();

    let mut harness = egui_kittest::Harness::new_ui_state(views::models::show, state);
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label("Inventariar").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("CALCULANDO SHA-256");
    harness.get_by_label("× Cancelar").click_accesskit();
    harness.run_steps(5);
    harness.get_by_label_contains("hash cancelado; SHAs parciais mantidos");

    std::fs::remove_dir_all(&dir).ok();
}
