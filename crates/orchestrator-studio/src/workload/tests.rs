use super::*;

/// Parâmetro inválido barra SEM rede: nada despacha, motivo honesto.
#[test]
fn invalid_params_block_without_network() {
    for (temperature, max_tokens, timeout_ms, expect) in [
        ("quente", "256", "300000", "temperatura inválida"),
        ("0.7", "zero", "300000", "max_tokens inválido"),
        ("0.7", "0", "300000", "max_tokens inválido"),
        ("0.7", "9000", "300000", "max_tokens inválido"),
        ("0.7", "256", "muito", "timeout inválido"),
        ("0.7", "256", "0", "timeout inválido"),
        ("99.0", "256", "300000", "temperatura inválida"),
    ] {
        let mut state = DispatchState::new();
        state.temperature = String::from(temperature);
        state.max_tokens = String::from(max_tokens);
        state.timeout_ms = String::from(timeout_ms);
        state.prompt = String::from("oi");
        state.send();
        assert!(
            !state.busy,
            "nada em voo com parâmetro inválido ({temperature}/{max_tokens}/{timeout_ms})"
        );
        assert!(
            state.result.contains(expect),
            "motivo honesto, recebido: {}",
            state.result
        );
    }
}

/// RESET zera sessão; Re-enviar restaura modelo+prompt do registro.
#[test]
fn reset_clears_and_resend_restores() {
    let mut state = DispatchState::new();
    state.history.push(DispatchRecord {
        task_id: String::from("t-9"),
        agent: None,
        latency_ms: None,
        ok: false,
        detail: String::from("x"),
        queue_ms: None,
        inference_ms: None,
        timestamp_ms: 1,
        model: String::from("mod-r"),
        prompt: String::from("prompt-r"),
    });
    // Porta 0: connect impossível — o worker falha sem pacotes; o
    // RESET órfão antes de qualquer poll (nada polui o histórico).
    state.url = String::from("http://127.0.0.1:0/");
    state.resend(0);
    assert_eq!(state.model, "mod-r");
    assert_eq!(state.prompt, "prompt-r");
    assert!(state.busy, "resend válido entra em voo");
    state.reset();
    assert!(state.history.is_empty());
    assert!(state.last.is_none());
    assert!(state.result.is_empty());
    assert!(!state.busy);
    state.resend(99); // índice inexistente: no-op, sem panic.
}
