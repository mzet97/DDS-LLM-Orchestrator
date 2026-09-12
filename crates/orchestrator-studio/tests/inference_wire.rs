//! Cliente de inferência contra servidor HTTP real em porta efêmera.
//!
//! Stub de fio (não mock de SDK): prova que temperatura/limite atravessam o
//! contrato e que o cliente lê modelos e trata escolhas vazias.

use std::sync::{Arc, Mutex};

use axum::{
    routing::{get, post},
    Json, Router,
};
use orchestrator_studio::inference::{ChatRequest, InferenceError, Message, Role};

type Captured = Arc<Mutex<Vec<serde_json::Value>>>;

fn stub(captured: Captured) -> Router {
    Router::new()
        .route(
            "/v1/models",
            get(|| async {
                Json(serde_json::json!({"data": [{"id": "modelo-teste"}]}))
            }),
        )
        .route(
            "/v1/chat/completions",
            post(|Json(body): Json<serde_json::Value>| async move {
                captured
                    .lock()
                    .expect("captura acessivel")
                    .push(body.clone());
                let empty = body["model"] == "vazio";
                Json(if empty {
                    serde_json::json!({"choices": []})
                } else {
                    serde_json::json!({"choices": [{"message": {"role": "assistant", "content": "OK-prova"}}]})
                })
            }),
        )
}

async fn live_base_url(captured: Captured) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    tokio::spawn(async move {
        axum::serve(listener, stub(captured))
            .await
            .expect("stub de teste serve");
    });
    format!("http://{addr}")
}

fn chat(model: &str) -> ChatRequest {
    ChatRequest {
        model: String::from(model),
        messages: vec![Message {
            role: Role::User,
            content: String::from("diga OK"),
        }],
        temperature: 0.7,
        max_tokens: 11,
    }
}

#[tokio::test]
async fn lists_models_from_wire() {
    let url = live_base_url(Captured::default()).await;

    let models =
        tokio::task::spawn_blocking(move || orchestrator_studio::inference::list_models(&url))
            .await
            .expect("sem panic")
            .expect("stub responde");

    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "modelo-teste");
}

#[tokio::test]
async fn parameters_cross_the_wire_and_content_returns() {
    let captured = Captured::default();
    let url = live_base_url(captured.clone()).await;

    let content = tokio::task::spawn_blocking(move || {
        orchestrator_studio::inference::chat_completion(&url, &chat("m"))
    })
    .await
    .expect("sem panic")
    .expect("stub responde");

    assert_eq!(content, "OK-prova");
    let bodies = captured.lock().expect("captura acessivel");
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0]["temperature"], serde_json::json!(0.7));
    assert_eq!(bodies[0]["max_tokens"], serde_json::json!(11));
    assert_eq!(bodies[0]["model"], serde_json::json!("m"));
    assert_eq!(
        bodies[0]["messages"][0]["content"],
        serde_json::json!("diga OK")
    );
}

#[tokio::test]
async fn session_accumulates_history_on_the_wire() {
    use orchestrator_studio::inference::InferenceState;

    let captured = Captured::default();
    let url = live_base_url(captured.clone()).await;
    let mut session = InferenceState::new();
    session.server_url = url.clone();
    session.model = String::from("m");
    session.prompt = String::from("primeira");

    tokio::task::spawn_blocking(move || {
        session.send();
        session.prompt = String::from("segunda");
        session.send();
        session
    })
    .await
    .expect("sem panic");

    let bodies = captured.lock().expect("captura acessivel");
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0]["messages"].as_array().expect("lista").len(), 1);
    let second = bodies[1]["messages"].as_array().expect("lista");
    assert_eq!(second.len(), 3);
    assert_eq!(second[1]["role"], serde_json::json!("assistant"));
    assert_eq!(second[2]["content"], serde_json::json!("segunda"));
}

/// Smoke contra llama-server real: só roda com `STUDIO_LIVE_LLAMA=1`
/// (padrão dos gates de middleware: bloqueado, nunca aprovado por fixture).
#[test]
fn live_llama_smoke_when_requested() {
    if std::env::var("STUDIO_LIVE_LLAMA").is_err() {
        return;
    }
    let url = "http://127.0.0.1:8082";
    let models = orchestrator_studio::inference::list_models(url).expect("llama vivo lista");
    assert!(
        !models.is_empty(),
        "servidor real anuncia ao menos um modelo"
    );
    // Conteúdo pode vir vazio com budget curto em modelo reasoning (visto em
    // curl: `reasoning_content` consome os tokens); o contrato do cliente é
    // completar sem erro e a travessia de parâmetros já é provada no stub.
    let mut request = chat(&models[0].id);
    request.max_tokens = 64;
    let _reply: String =
        orchestrator_studio::inference::chat_completion(url, &request).expect("llama vivo gera");
}

#[tokio::test]
async fn empty_choices_become_typed_error() {
    let url = live_base_url(Captured::default()).await;

    let err = tokio::task::spawn_blocking(move || {
        orchestrator_studio::inference::chat_completion(&url, &chat("vazio"))
    })
    .await
    .expect("sem panic")
    .expect_err("escolhas vazias devem falhar");

    assert!(matches!(err, InferenceError::EmptyChoices));
}
