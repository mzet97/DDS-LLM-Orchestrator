//! Parte de `runner` (extraída para manter os arquivos revisáveis).

use super::*;

/// Linha de build do executável (`--version` real, 3 s de orçamento):
pub(super) fn spawn_reader(
    stream: Option<impl Read + Send + 'static>,
    prefix: &'static str,
    tx: &mpsc::Sender<RunnerMsg>,
) {
    let Some(stream) = stream else { return };
    let tx = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let text = if prefix.is_empty() {
                line
            } else {
                format!("{prefix}{line}")
            };
            if tx.send(RunnerMsg::Line(text)).is_err() {
                break;
            }
        }
    });
}

/// As 5 etapas canônicas (PRD 3.4) sobre o subprocesso LOCAL: ① validar,
/// ② spawn, ③ `/health`, ④ `/v1/models`, ⑤ prova de geração. Roda em worker;
/// `cancel` aborta entre tentativas (Parar durante a subida).
pub(super) fn run_steps(
    params: RunnerParams,
    spawner: impl FnOnce(&RunnerParams) -> std::io::Result<Child>,
    tx: mpsc::Sender<RunnerMsg>,
    cancel: Arc<AtomicBool>,
) {
    use std::time::Instant;
    let timed = |step: StepResult, started: Instant| {
        let mut step = step;
        step.duration_ms = started.elapsed().as_millis() as u64;
        step
    };
    let fail = |step: &'static str, detail: String| StepResult {
        step,
        ok: false,
        detail,
        duration_ms: 0,
    };
    let done = |step: &'static str, detail: String| StepResult {
        step,
        ok: true,
        detail,
        duration_ms: 0,
    };
    let cancelled = || cancel.load(Ordering::Relaxed);

    // ① Validar binário + modelo + porta (+ build real).
    let t0 = Instant::now();
    let build = probe_build(&params.exe);
    let model_bytes = std::fs::metadata(params.model.trim())
        .map(|meta| meta.len())
        .unwrap_or(0);
    if cancelled() {
        let _ = tx.send(RunnerMsg::Finished);
        return;
    }
    let _ = tx.send(RunnerMsg::Step(timed(
        done(
            "① validar binário + modelo + porta",
            format!(
                "{} · modelo {:.2} GB · porta {} livre · build: {}",
                params.exe,
                model_bytes as f64 / 1_000_000_000.0,
                params.port,
                build
            ),
        ),
        t0,
    )));

    // ② Spawn + leitores de stdout/stderr.
    let t1 = Instant::now();
    let mut child = match spawner(&params) {
        Ok(child) => child,
        Err(err) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail("② spawn do processo", err.to_string()),
                t1,
            )));
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
    };
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let _ = tx.send(RunnerMsg::Spawned { child, pid });
    spawn_reader(stdout, "", &tx);
    spawn_reader(stderr, "[stderr] ", &tx);
    let _ = tx.send(RunnerMsg::Step(timed(
        done("② spawn do processo", format!("PID {pid} ativo")),
        t1,
    )));

    let base = params.base_url();

    // ③ Aguardar `/health` (200).
    let t2 = Instant::now();
    let health_budget = params.health_timeout_secs.max(1);
    let deadline = t2 + std::time::Duration::from_secs(health_budget);
    let mut healthy_ms = None;
    while Instant::now() < deadline {
        if cancelled() {
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
        match health_once(&base) {
            Ok(ms) => {
                healthy_ms = Some(ms);
                break;
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(500)),
        }
    }
    let Some(healthy_ms) = healthy_ms else {
        let _ = tx.send(RunnerMsg::Step(timed(
            fail("③ aguardar /health", format!("sem 200 em {health_budget}s")),
            t2,
        )));
        let _ = tx.send(RunnerMsg::Finished);
        return;
    };
    let _ = tx.send(RunnerMsg::Step(timed(
        done("③ aguardar /health", format!("200 OK em {healthy_ms}ms")),
        t2,
    )));

    // ④ Modelo alocado (`/v1/models` não vazio).
    let t3 = Instant::now();
    if cancelled() {
        let _ = tx.send(RunnerMsg::Finished);
        return;
    }
    let model = match list_models(&base) {
        Ok(models) if !models.is_empty() => {
            let id = models[0].id.clone();
            let _ = tx.send(RunnerMsg::Step(timed(
                done("④ modelo & KV cache (/v1/models)", format!("modelo: {id}")),
                t3,
            )));
            id
        }
        Ok(_) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail(
                    "④ modelo & KV cache (/v1/models)",
                    String::from("servidor sem modelos anunciados"),
                ),
                t3,
            )));
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
        Err(err) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail("④ modelo & KV cache (/v1/models)", err.to_string()),
                t3,
            )));
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
    };

    // ⑤ Prova de geração real.
    let t4 = Instant::now();
    if cancelled() {
        let _ = tx.send(RunnerMsg::Finished);
        return;
    }
    let proof = chat_completion(
        &base,
        &ChatRequest {
            model,
            messages: vec![Message {
                role: Role::User,
                content: String::from(PROOF_PROMPT),
                ts_unix_ms: crate::machines::now_unix_ns() / 1_000_000,
            }],
            temperature: 0.0,
            top_p: 1.0,
            max_tokens: 32,
        },
    );
    match proof {
        Ok(reply) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                done("⑤ prova de geração", reply),
                t4,
            )));
        }
        Err(err) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail("⑤ prova de geração", err.to_string()),
                t4,
            )));
        }
    }
    let _ = tx.send(RunnerMsg::Finished);
}
