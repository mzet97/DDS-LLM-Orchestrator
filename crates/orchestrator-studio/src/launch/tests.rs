use super::*;

fn known() -> Vec<String> {
    vec![String::from("llama-server")]
}

#[test]
fn empty_service_never_actuates() {
    let plan = LaunchPlan::new();

    let err = plan.validate(&known()).expect_err("vazio deve falhar");

    assert!(matches!(err, LaunchError::Invalid { .. }));
}

#[test]
fn unknown_service_is_rejected_before_any_effect() {
    let mut plan = LaunchPlan::new();
    plan.service = String::from("unidade-fantasma");

    let err = plan
        .validate(&known())
        .expect_err("desconhecida deve falhar");

    assert!(err.to_string().contains("lista lida do nó"));
}

#[test]
fn preview_states_everything_before_effect() {
    let mut plan = LaunchPlan::new();
    plan.service = String::from("llama-server");

    let preview = plan.preview();

    assert!(preview.contains("POST"));
    assert!(preview.contains("llama-server"));
    assert!(preview.contains("NADA é criado"));
}

#[test]
fn stub_runner_drives_steps_through_poll() {
    let mut state = LaunchState::new();
    state.plan.service = String::from("llama-server");
    state.start_with(&known(), |_| {
        vec![
            StepResult {
                step: "atuar start",
                ok: true,
                detail: String::from("ok"),
                duration_ms: 0,
            },
            StepResult {
                step: "prova de geração",
                ok: true,
                detail: String::from("INFERENCIA_OK"),
                duration_ms: 0,
            },
        ]
    });

    for _ in 0..1000 {
        state.poll();
        if !state.running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert!(state.proved());
    assert_eq!(state.steps.len(), 2);
}

#[test]
fn failing_step_surfaces_first_error() {
    let mut state = LaunchState::new();
    state.plan.service = String::from("llama-server");
    state.start_with(&known(), |_| {
        vec![StepResult {
            step: "atuar start",
            ok: false,
            detail: String::from("conexão recusada"),
            duration_ms: 0,
        }]
    });

    for _ in 0..1000 {
        state.poll();
        if !state.running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert!(!state.proved());
    assert!(state.error.contains("conexão recusada"));
}
