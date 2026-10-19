//! Parte de `inference` (extraída para manter os arquivos revisáveis).

use super::*;

/// Lista os modelos do servidor (`GET /v1/models`).
pub fn list_models(base_url: &str) -> Result<Vec<ModelInfo>, InferenceError> {
    let url = base_url.trim_end_matches('/');
    let response = send(url, client(url)?.get(format!("{url}/v1/models")))?;
    #[derive(Deserialize)]
    struct List {
        data: Vec<ModelInfo>,
    }
    response
        .json::<List>()
        .map(|list| list.data)
        .map_err(|err| InferenceError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

/// Geração real (`POST /v1/chat/completions`); retorna o conteúdo da 1ª
/// escolha (mantida para os callers que só querem o texto).
pub fn chat_completion(base_url: &str, chat: &ChatRequest) -> Result<String, InferenceError> {
    chat_completion_with_stats(base_url, chat).map(|(content, _)| content)
}

/// Como [`chat_completion`], devolvendo também o `usage` reportado pelo
/// servidor (tokens) quando presente na resposta.
pub fn chat_completion_with_stats(
    base_url: &str,
    chat: &ChatRequest,
) -> Result<(String, Option<UsageInfo>), InferenceError> {
    let url = base_url.trim_end_matches('/');
    let response = send(
        url,
        client(url)?
            .post(format!("{url}/v1/chat/completions"))
            .json(chat),
    )?;
    #[derive(Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(Deserialize)]
    struct Completion {
        choices: Vec<Choice>,
        #[serde(default)]
        usage: Option<UsageInfo>,
    }
    let completion: Completion = response.json().map_err(|err| InferenceError::Unreachable {
        url: String::from(url),
        detail: err.to_string(),
    })?;
    completion
        .choices
        .into_iter()
        .next()
        .map(|choice| (choice.message.content, completion.usage))
        .ok_or(InferenceError::EmptyChoices)
}
