//! Parte de `launch` (extraída para manter os arquivos revisáveis).

use super::*;

pub(crate) fn run_launch(plan: LaunchPlan) -> Vec<StepResult> {
    use std::time::Instant;
    let mut steps = Vec::with_capacity(5);
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

    // ① Verificar unidade & binário no nó (leitura pura, sem efeito).
    let t0 = Instant::now();
    match list_services(&plan.node_url) {
        Ok(list) if list.iter().any(|item| item.service == plan.service) => {
            let known: Vec<&str> = list.iter().map(|item| item.service.as_str()).collect();
            steps.push(timed(
                done(
                    "① verificar unidade & binário",
                    format!(
                        "unidade {} presente no nó ({})",
                        plan.service,
                        known.join(" · ")
                    ),
                ),
                t0,
            ));
        }
        Ok(list) => {
            steps.push(timed(
                fail(
                    "① verificar unidade & binário",
                    format!(
                        "unidade {} ausente no nó — unidades: {}",
                        plan.service,
                        list.iter()
                            .map(|item| item.service.as_str())
                            .collect::<Vec<_>>()
                            .join(" · ")
                    ),
                ),
                t0,
            ));
            return steps;
        }
        Err(err) => {
            steps.push(timed(
                fail("① verificar unidade & binário", err.to_string()),
                t0,
            ));
            return steps;
        }
    }

    // ② Spawn do processo (systemd start).
    let t1 = Instant::now();
    match actuate(
        &plan.node_url,
        &plan.service,
        true,
        &fresh_operation_id(&plan.service),
    ) {
        Ok(out) if out.active || !out.acted => steps.push(timed(
            done(
                "② spawn do processo (systemd start)",
                format!("wanted=true active={} acted={}", out.active, out.acted),
            ),
            t1,
        )),
        Ok(out) => steps.push(timed(
            done(
                "② spawn do processo (systemd start)",
                format!("registrado; active={} (aguardando)", out.active),
            ),
            t1,
        )),
        Err(err) => {
            steps.push(timed(
                fail("② spawn do processo (systemd start)", err.to_string()),
                t1,
            ));
            return steps;
        }
    }

    // ③ Aguardar o processo vivo (active=true).
    let t2 = Instant::now();
    let mut active = false;
    for _ in 0..60 {
        match list_services(&plan.node_url) {
            Ok(list)
                if list
                    .iter()
                    .any(|item| item.service == plan.service && item.active) =>
            {
                active = true;
                break;
            }
            Ok(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
            Err(err) => {
                steps.push(timed(fail("③ aguardar processo vivo", err.to_string()), t2));
                return steps;
            }
        }
    }
    if !active {
        steps.push(timed(
            fail(
                "③ aguardar processo vivo",
                String::from("unidade não ficou ativa em 60s"),
            ),
            t2,
        ));
        return steps;
    }
    steps.push(timed(
        done("③ aguardar processo vivo", String::from("active=true")),
        t2,
    ));

    // ④ Modelo & KV cache (o servidor anuncia o modelo alocado pela unidade).
    let t3 = Instant::now();
    let model = match list_models(&plan.llama_url) {
        Ok(models) if !models.is_empty() => {
            let id = models[0].id.clone();
            steps.push(timed(
                done(
                    "④ modelo & KV cache (/v1/models)",
                    format!("modelo alocado: {id}"),
                ),
                t3,
            ));
            id
        }
        Ok(_) => {
            steps.push(timed(
                fail(
                    "④ modelo & KV cache (/v1/models)",
                    String::from("servidor sem modelos anunciados"),
                ),
                t3,
            ));
            return steps;
        }
        Err(err) => {
            steps.push(timed(
                fail("④ modelo & KV cache (/v1/models)", err.to_string()),
                t3,
            ));
            return steps;
        }
    };

    // ⑤ Prova de geração real (a publicação ServerStatus é visível na
    // descoberta — a view mostra o chip ao vivo).
    let t4 = Instant::now();
    match chat_completion(
        &plan.llama_url,
        &ChatRequest {
            model,
            messages: vec![Message {
                role: Role::User,
                content: plan.proof_prompt.clone(),
                ts_unix_ms: crate::machines::now_unix_ns() / 1_000_000,
            }],
            temperature: 0.0,
            top_p: 1.0,
            max_tokens: 32,
        },
    ) {
        Ok(reply) => steps.push(timed(done("⑤ prova de geração", reply), t4)),
        Err(err) => steps.push(timed(fail("⑤ prova de geração", err.to_string()), t4)),
    }
    steps
}
