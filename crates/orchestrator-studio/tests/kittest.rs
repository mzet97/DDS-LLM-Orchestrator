//! Testes de UI automatizados com `egui_kittest` (T-890-08, G-37).
//!
//! `default-features = false` no dev-dep: SEM wgpu/snapshot — os testes são
//! de INTERAÇÃO pela árvore accesskit (clicar botões, ler rótulos), 100%
//! headless, sem GPU e sem diffs de imagem. Painéis reais da GUI são
//! renderizados e operados como o usuário faria.

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::views;

fn temp_models_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "studio-kittest-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir temporário");
    dir
}

/// Fluxo real do painel de Modelos: inventariar → carregar manifesto →
/// status OK verde na tabela (G-09..11 exercitado pela UI).
#[test]
fn models_panel_inventory_and_manifest_ok_flow() {
    let dir = temp_models_dir("ok");
    std::fs::write(dir.join("qwen.gguf"), b"conteudo do gguf congelado").expect("escreve gguf");
    let expected_sha = {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"conteudo do gguf congelado");
        format!("{:x}", hasher.finalize())
    };
    let manifest_path = dir.join("models-manifest.json");
    std::fs::write(
        &manifest_path,
        format!("{{\"models\": {{\"qwen.gguf\": \"{expected_sha}\"}}}}"),
    )
    .expect("escreve manifesto");

    let mut state = ModelsState::new();
    state.dir = dir.clone();
    state.manifest_path = manifest_path.display().to_string();

    let mut harness = egui_kittest::Harness::new_ui_state(views::models::show, state);
    harness.set_size(egui::vec2(900.0, 500.0));

    // Inventaria (o hash do arquivo minúsculo conclui em poucos frames).
    harness.get_by_label("Inventariar").click();
    harness.run_steps(30);

    // Carrega o manifesto e recomputa os status.
    harness.get_by_label("Carregar manifesto").click();
    harness.run_steps(10);

    harness.get_by_label("manifesto carregado: 1 registro(s)"); // entra em pânico com mensagem se ausente
    harness.get_by_label("OK"); // pânico com mensagem se ausente

    // Limpeza depois do harness (o state morre com ele).
    std::fs::remove_dir_all(&dir).ok();
}

/// Desvio detectado PELA UI: arquivo adulterado → status DESVIADO (vermelho).
#[test]
fn models_panel_flags_deviant_artifact() {
    let dir = temp_models_dir("deviant");
    std::fs::write(dir.join("qwen.gguf"), b"arquivo adulterado").expect("escreve gguf");
    let manifest_path = dir.join("models-manifest.json");
    std::fs::write(
        &manifest_path,
        "{\"models\": {\"qwen.gguf\": \"bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517\"}}",
    )
    .expect("escreve manifesto");

    let mut state = ModelsState::new();
    state.dir = dir.clone();
    state.manifest_path = manifest_path.display().to_string();

    let mut harness = egui_kittest::Harness::new_ui_state(views::models::show, state);
    harness.set_size(egui::vec2(900.0, 500.0));

    harness.get_by_label("Inventariar").click();
    harness.run_steps(30);
    harness.get_by_label("Carregar manifesto").click();
    harness.run_steps(10);

    harness.get_by_label("DESVIADO"); // entra em pânico com mensagem se ausente
    std::fs::remove_dir_all(&dir).ok();
}

/// Painel Workflow sem a feature `dds`: orientação honesta de compilação.
#[cfg(not(feature = "dds"))]
#[test]
fn workflow_panel_without_dds_shows_guidance() {
    let state = orchestrator_studio::workflow::WorkflowState::new();
    let mut harness = egui_kittest::Harness::new_ui_state(views::workflow::show, state);
    harness.set_size(egui::vec2(700.0, 300.0));
    harness.run_steps(5);
    harness.get_by_label_contains("Requer a feature `dds`"); // pânico se ausente
}
