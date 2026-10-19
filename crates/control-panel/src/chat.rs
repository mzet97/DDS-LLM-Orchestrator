//! Construção de mensagens de chat (paridade com `chat_client.py`, hoje quebrado
//! no Python por importar `orchestrator.models`). Funções puras; o transporte
//! DDS entra pela trait `ChatTransport` (crate `client` no E2E).

use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ChatError {
    #[error("tipo de arquivo nao suportado: {0}")]
    UnsupportedFileType(String),
    #[error("prompt vazio")]
    EmptyPrompt,
}

/// Especialização exigida (ModelSpecialization do contrato Python).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Specialization {
    Text,
    Vision,
    Transcription,
}

impl Specialization {
    pub fn as_str(&self) -> &'static str {
        match self {
            Specialization::Text => "TEXT",
            Specialization::Vision => "VISION",
            Specialization::Transcription => "TRANSCRIPTION",
        }
    }
}

/// Arquivo anexado ao chat.
#[derive(Debug, Clone)]
pub struct ChatFile {
    pub name: String,
    pub mime_type: String,
    pub data_base64: String,
}

const IMAGE_TYPES: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/jpg",
    "image/gif",
    "image/webp",
];
const AUDIO_TYPES: [&str; 6] = [
    "audio/wav",
    "audio/x-wav",
    "audio/mpeg",
    "audio/mp3",
    "audio/ogg",
    "audio/webm",
];

/// Monta o payload JSON de mensagens (texto puro ou visão com imagens).
pub fn build_messages_json(
    prompt: &str,
    spec: Specialization,
    files: &[ChatFile],
) -> Result<String, ChatError> {
    if prompt.trim().is_empty() && files.is_empty() {
        return Err(ChatError::EmptyPrompt);
    }
    if spec == Specialization::Transcription {
        return build_transcription_payload(prompt, files);
    }
    let mut content: Vec<Value> = vec![json!({"type": "text", "text": prompt})];
    for file in files {
        if IMAGE_TYPES.contains(&file.mime_type.as_str()) {
            content.push(json!({
                "type": "image_url",
                "image_url": {"url": format!("data:{};base64,{}", file.mime_type, file.data_base64)},
            }));
        } else if AUDIO_TYPES.contains(&file.mime_type.as_str()) {
            return Err(ChatError::UnsupportedFileType(format!(
                "audio requer especializacao TRANSCRIPTION: {}",
                file.mime_type
            )));
        } else {
            return Err(ChatError::UnsupportedFileType(file.mime_type.clone()));
        }
    }
    let messages = json!([{"role": "user", "content": content}]);
    Ok(serde_json::to_string(&messages).unwrap_or_default())
}

/// Monta o payload de transcrição (áudio obrigatório).
pub fn build_transcription_payload(prompt: &str, files: &[ChatFile]) -> Result<String, ChatError> {
    let audio = files
        .iter()
        .find(|f| AUDIO_TYPES.contains(&f.mime_type.as_str()))
        .ok_or_else(|| {
            ChatError::UnsupportedFileType("transcricao requer arquivo de audio".into())
        })?;
    let payload = json!({
        "task_type": "transcription",
        "prompt": prompt,
        "audio": {"name": audio.name, "mime_type": audio.mime_type, "data_base64": audio.data_base64},
    });
    Ok(serde_json::to_string(&payload).unwrap_or_default())
}

/// Transporte de chat: implementado pelo `client` DDS no E2E; testes usam fake.
pub trait ChatTransport: Send + Sync {
    /// Envia o payload e devolve os chunks de resposta concatenados.
    fn send(&self, payload: &str, spec: Specialization, timeout_s: f64) -> Result<String, String>;
}

/// Transporte real via crate `client` (DDS): cria a Task PENDING com o
/// payload de mensagens e aguarda o resultado (feature `dds`).
#[cfg(feature = "dds")]
pub struct ClientChatTransport {
    domain: u32,
    client_id: String,
}

#[cfg(feature = "dds")]
impl ClientChatTransport {
    pub fn new(domain: u32, client_id: impl Into<String>) -> Self {
        Self {
            domain,
            client_id: client_id.into(),
        }
    }

    /// Envia o payload e devolve o conteúdo do resultado.
    ///
    /// P2 (review PR#10): especialização exigida e limite de tokens seguem
    /// na Task — antes o spec só moldava o messages_json (request de VISION/
    /// TRANSCRIPTION ia para agente TEXT) e o max_tokens escolhido na UI era
    /// descartado (Task fixa em 256).
    pub async fn send_async(
        &self,
        payload: &str,
        spec: Specialization,
        max_tokens: u32,
        timeout_s: f64,
    ) -> Result<String, String> {
        use client::{ClientConfig, DdsClient};
        let config = ClientConfig {
            client_id: self.client_id.clone(),
            dds_domain: self.domain,
            timeout_ms: (timeout_s * 1000.0) as u64,
        };
        let factory = DdsClient::new(config.clone());
        let mut task = factory.create_task("", payload, 5, false);
        // ordinais do contrato (dds_types.py): 0=TEXT 1=VISION 2=EMBEDDING
        // 3=TRANSCRIPTION — cast direto do enum local alinharia errado.
        task.model_required = match spec {
            Specialization::Text => 0,
            Specialization::Vision => 1,
            Specialization::Transcription => 3,
        };
        task.max_tokens = max_tokens;
        let transport = client::dds_impl::DdsClientDds::new(config).map_err(|e| e.to_string())?;
        let result = transport.submit(task).await.map_err(|e| e.to_string())?;
        if result.success {
            Ok(result.content)
        } else {
            Err(format!("task {} falhou", result.task_id))
        }
    }
}

/// Lê um arquivo e o converte em `ChatFile` (file_to_dict do Python).
pub fn file_to_dict(path: &std::path::Path) -> Result<ChatFile, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let mime = mime_from_extension(path);
    Ok(ChatFile {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        mime_type: mime,
        data_base64: base64_encode(&bytes),
    })
}

fn mime_from_extension(path: &std::path::Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "webm" => "audio/webm",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Base64 padrão (sem dependência externa).
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
