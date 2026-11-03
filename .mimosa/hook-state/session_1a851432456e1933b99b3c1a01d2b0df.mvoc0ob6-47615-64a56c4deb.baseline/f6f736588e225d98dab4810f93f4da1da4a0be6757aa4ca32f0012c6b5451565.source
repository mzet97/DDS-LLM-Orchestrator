//! 3.4 Subir Inferência: verificação headless do runner LOCAL — formulário,
//! presets, CLI preview, faixa PID/VRAM, wizard de etapas, console e chip
//! ServerStatus. O ciclo de vida usa binários reais (`/bin/echo` para
//! spawn/saída/exit, `/bin/sleep` para o SIGTERM): PID/saída/exit
//! verdadeiros, sem llama.

use std::process::{Command, Stdio};
use std::time::Duration;

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::discovery::DiscoveryState;
use orchestrator_studio::runner::RunnerState;
use orchestrator_studio::views;

/// Arquivo GGUF vazio (só para a validação "modelo existe").
fn fake_gguf(name: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, b"GGUF-falso").expect("temp escreve");
    path.to_string_lossy().into_owned()
}

/// Porta livre no momento (TOCTOU aceitável em teste).
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("loopback liga")
        .local_addr()
        .expect("addr")
        .port()
}

fn poll_until(runner: &mut RunnerState, secs: u64, done: impl Fn(&RunnerState) -> bool) {
    for _ in 0..secs * 20 {
        runner.poll();
        if done(runner) {
            runner.poll();
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    runner.poll();
}

/// Estado zerado: formulário, presets, faixa, wizard vazio, console vazia.
#[test]
fn launch_renders_idle_form_steps_and_console() {
    let mut runner = RunnerState::new();
    let mut go_chat = false;
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (runner, go_chat, discovery)| views::launch::show(ui, runner, go_chat, discovery),
        (&mut runner, &mut go_chat, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label_contains("3.4 Subir Inferência");
    harness.get_by_label_contains("SUBPROCESS STATE");
    harness.get_by_label_contains("STATUS: PARADO");
    harness.get_by_label_contains("TARGET ACCELERATOR");
    harness.get_by_label_contains("VRAM ALOCADA");
    harness.get_by_label_contains("Parâmetros de Inicialização");
    harness.get_by_label_contains("EXECUTÁVEL LLAMA-SERVER");
    harness.get_by_label_contains("ARTEFATO DE MODELO GGUF (DISCO)");
    harness.get_by_label_contains("PRESETS DE PERFIL OPERACIONAL");
    harness.get_by_label_contains("Alta Precisão");
    harness.get_by_label_contains("Baixa Latência");
    harness.get_by_label_contains("Codificação / Agente");
    harness.get_by_label_contains("PORTA HTTP");
    harness.get_by_label_contains("BIND ADDRESS");
    harness.get_by_label_contains("COMANDO GERADO (CLI PREVIEW)");
    harness.get_by_label_contains("SUBIR LLAMA-SERVER LOCAL");
    harness.get_by_label_contains("ETAPAS DE INICIALIZAÇÃO DO RUNNER");
    harness.get_by_label_contains("nenhuma execução ainda");
    harness.get_by_label_contains("LIVE OUTPUT: LLAMA-SERVER STDOUT / STDERR");
    harness.get_by_label_contains("console vazia");
    harness.get_by_label_contains("SERVERSTATUS NO DOMÍNIO: nenhum anúncio");
    harness.get_by_label("Parar (SIGTERM)");
    harness.get_by_label("Reiniciar");
    harness.get_by_label("Abrir no Chat (3.3)");
}

/// Ciclo com `/bin/echo`: spawn real, saída capturada, exit 0 observado e
/// etapa ③ falhando honesta (nada escuta a porta).
#[test]
fn launch_echo_lifecycle_renders_steps_console_and_failure() {
    let mut runner = RunnerState::new();
    runner.params.exe = String::from("/bin/echo");
    runner.params.model = fake_gguf("kittest_runner_modelo.gguf");
    runner.params.bind = String::from("127.0.0.1");
    runner.params.port = free_port();
    runner.params.health_timeout_secs = 2;
    runner.start_with(|_| {
        Command::new("/bin/echo")
            .arg("saida-runner-prova")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    });
    poll_until(&mut runner, 10, |runner| {
        runner.steps.len() == 3
            && runner
                .console
                .iter()
                .any(|line| line.contains("processo encerrado"))
    });
    assert_eq!(runner.steps.len(), 3, "esperava ①② ok + ③ falha");
    assert!(runner.steps[0].ok && runner.steps[1].ok && !runner.steps[2].ok);

    let mut go_chat = false;
    let discovery = DiscoveryState::new_disabled(170);
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (runner, go_chat, discovery)| views::launch::show(ui, runner, go_chat, discovery),
        (&mut runner, &mut go_chat, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label("① validar binário + modelo + porta");
    harness.get_by_label("② spawn do processo");
    harness.get_by_label_contains("PID");
    harness.get_by_label("③ aguardar /health");
    harness.get_by_label("sem 200 em 2s");
    harness.get_by_label_contains("saida-runner-prova");
    harness.get_by_label_contains("processo encerrado (exit 0)");
    harness.get_by_label_contains("STATUS: FALHOU");
    harness.get_by_label_contains("2/5 CONCLUÍDOS");
}

/// Combo de GGUF com diretório real: abre, lista, seleciona e o SHA-256 do
/// arquivo pequeno aparece (worker de hash de verdade).
#[test]
fn launch_model_combo_selects_and_hashes() {
    let dir = std::env::temp_dir().join("kittest_runner_models");
    std::fs::create_dir_all(&dir).expect("temp cria dir");
    std::fs::write(dir.join("a.gguf"), b"conteudo-a").expect("temp escreve");
    std::fs::write(dir.join("b.gguf"), b"conteudo-b").expect("temp escreve");

    let mut runner = RunnerState::new();
    runner.models_dir = dir.to_string_lossy().into_owned();
    runner.refresh_models();
    assert_eq!(runner.models.len(), 2, "2 GGUFs no dir");

    let mut go_chat = false;
    let discovery = DiscoveryState::new_disabled(170);
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (runner, go_chat, discovery)| views::launch::show(ui, runner, go_chat, discovery),
        (&mut runner, &mut go_chat, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_value("— escolha o GGUF —").click();
    harness.run_steps(3);
    harness.get_by_label_contains("a.gguf (").click();
    harness.run_steps(20);
    // O hash só roda para o modelo SELECIONADO: "SHA-256 OK" prova a
    // seleção de ponta a ponta (o Path repete o texto em 2 nós).
    harness.get_by_label_contains("SHA-256 OK");
}

/// SIGTERM real: `/bin/sleep` parado via [`RunnerState::stop`] sai e a UI
/// volta a PARADO (transição de estado, sem janela).
#[test]
fn launch_stop_sends_sigterm_and_reaps_child() {
    let mut runner = RunnerState::new();
    runner.params.exe = String::from("/bin/sleep");
    runner.params.model = fake_gguf("kittest_runner_modelo2.gguf");
    runner.params.bind = String::from("127.0.0.1");
    runner.params.port = free_port();
    runner.params.health_timeout_secs = 120;
    runner.start_with(|_| {
        Command::new("/bin/sleep")
            .arg("120")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    });
    poll_until(&mut runner, 5, |runner| runner.pid().is_some());
    assert!(runner.pid().is_some(), "sleep spawnado tem PID");
    runner.stop();
    poll_until(&mut runner, 10, |runner| {
        matches!(
            runner.status,
            orchestrator_studio::runner::RunnerStatus::Stopped
        )
    });
    assert!(
        matches!(
            runner.status,
            orchestrator_studio::runner::RunnerStatus::Stopped
        ),
        "SIGTERM ceifado volta a PARADO: {:?}",
        runner.status
    );
    assert!(
        runner
            .console
            .iter()
            .any(|line| line.contains("SIGTERM enviado")),
        "console registra o SIGTERM"
    );
}
