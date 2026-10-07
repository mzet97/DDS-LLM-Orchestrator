//! Registro de eventos da GUI (pedido do autor 2026-10-06): analisar o
//! comportamento do Studio — descoberta, auto-carga, observação DDS —
//! sem depender só do stderr. Cada evento vai para o stderr E para um
//! buffer circular em memória exibido no painel "Logs".
//!
//! PRD 3.14: cada entrada carrega SUBSISTEMA de origem, payload JSON
//! serializado do contexto, fonte (file:line via `#[track_caller]`),
//! thread, offset (Δ ms vs entrada anterior) e — em ERRO — stack trace
//! capturado. Deliberadamente simples (sem tracing/subscriber): os sítios
//! de log são poucos e o objetivo é depuração operacional.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Capacidade do buffer (eventos mais antigos são descartados).
const CAPACITY: usize = 500;

/// Cap de retenção de stack trace/payload (inspetor legível, memória bornal).
const DETAIL_CAP: usize = 2 * 1024;

/// Um evento registrado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// Instante wall-clock legível (HH:MM:SS hora local).
    pub timestamp: String,
    pub level: &'static str,
    /// Subsistema de origem (SYSTEMD · DISCOVERY · CATALOG · DDS_RTPS ·
    /// HTTP · LAUNCH · GUI).
    pub source: &'static str,
    pub message: String,
    /// Payload serializado do contexto (JSON ou valor cru), quando há.
    pub payload: Option<String>,
    /// Fonte do sítio de chamada (`file:line`, via `#[track_caller]`).
    pub origin: String,
    /// Thread que registrou (`ThreadId(n)` formatado).
    pub thread: String,
    /// Δ ms desde a entrada anterior (0 na primeira).
    pub offset_ms: u64,
    /// Stack trace capturado (somente ERRO — `Backtrace::force_capture`).
    pub backtrace: Option<String>,
    /// Slot sequencial no ring buffer (1..=CAPACITY, para "SLOT #").
    pub slot: u64,
}

static ENTRIES: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());
static LAST_NS: AtomicU64 = AtomicU64::new(0);
static NEXT_SLOT: AtomicU64 = AtomicU64::new(1);

fn now_unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn capped(text: String) -> String {
    if text.len() <= DETAIL_CAP {
        text
    } else {
        format!("{}…", &text[..DETAIL_CAP])
    }
}

#[allow(clippy::too_many_arguments)]
fn push(
    level: &'static str,
    source: &'static str,
    payload: Option<String>,
    message: String,
    origin: String,
) {
    let now_ns = now_unix_ns();
    let timestamp = chrono_now(now_ns);
    let last = LAST_NS.swap(now_ns, Ordering::Relaxed);
    let offset_ms = now_ns.saturating_sub(last) / 1_000_000;
    // Sob teste: sem poluir o stderr do cargo com centenas de eventos.
    if !cfg!(test) {
        eprintln!("studio[{level}][{source}] {timestamp} {message}");
    }
    let backtrace = (level == "ERRO")
        .then(|| capped(format!("{:?}", std::backtrace::Backtrace::force_capture())));
    if let Ok(mut entries) = ENTRIES.lock() {
        if entries.len() == CAPACITY {
            entries.pop_front();
        }
        entries.push_back(LogEntry {
            timestamp,
            level,
            source,
            payload: payload.map(capped),
            message,
            origin,
            thread: format!("{:?}", std::thread::current().id()),
            offset_ms: if last == 0 { 0 } else { offset_ms },
            backtrace,
            slot: NEXT_SLOT.fetch_add(1, Ordering::Relaxed),
        });
    }
}

/// Hora local compacta (sem depender de chrono: HH:MM:SS do sistema).
fn chrono_now(secs_unix: u64) -> String {
    let secs = secs_unix;
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    format!("{h:02}:{m:02}:{s:02}")
}

/// Registra um evento informativo.
#[track_caller]
pub fn info(message: impl Into<String>) {
    push("INFO", "GUI", None, message.into(), origin());
}

/// Registra um aviso (falha recuperável).
#[track_caller]
pub fn warn(message: impl Into<String>) {
    push("WARN", "GUI", None, message.into(), origin());
}

/// Registra um erro (captura stack trace para o inspetor 3.14).
#[track_caller]
pub fn error(message: impl Into<String>) {
    push("ERRO", "GUI", None, message.into(), origin());
}

/// Registra um evento informativo com subsistema e payload de contexto.
#[track_caller]
pub fn info_at(source: &'static str, payload: Option<String>, message: impl Into<String>) {
    push("INFO", source, payload, message.into(), origin());
}

/// Registra um aviso com subsistema e payload de contexto.
#[track_caller]
pub fn warn_at(source: &'static str, payload: Option<String>, message: impl Into<String>) {
    push("WARN", source, payload, message.into(), origin());
}

/// Registra um erro com subsistema, payload e stack trace (3.14).
#[track_caller]
pub fn error_at(source: &'static str, payload: Option<String>, message: impl Into<String>) {
    push("ERRO", source, payload, message.into(), origin());
}

/// `file:line` do sítio de chamada (via `#[track_caller]`).
#[track_caller]
fn origin() -> String {
    let location = std::panic::Location::caller();
    format!("{}:{}", location.file(), location.line())
}

/// Snapshot dos eventos (mais antigos primeiro) para o painel.
#[must_use]
pub fn entries() -> Vec<LogEntry> {
    ENTRIES
        .lock()
        .map(|entries| entries.iter().cloned().collect())
        .unwrap_or_default()
}

/// Estado de apresentação do painel de Logs (tela 3.14): filtro por nível,
/// busca por substring, auto-scroll e entrada selecionada no inspetor.
#[derive(Debug, Default)]
pub struct LogsPanel {
    pub filter_level: Option<&'static str>,
    pub search: String,
    pub auto_scroll: bool,
    /// Slot da entrada selecionada no inspetor lateral (3.14).
    pub selected: Option<u64>,
}

impl LogsPanel {
    /// Padrão com auto-scroll ligado (tail de console).
    #[must_use]
    pub fn new() -> Self {
        Self {
            auto_scroll: true,
            ..Self::default()
        }
    }
}

/// Limpa o ring buffer (ação explícita do operador na tela 3.14).
pub fn clear() {
    if let Ok(mut entries) = ENTRIES.lock() {
        entries.clear();
    }
    LAST_NS.store(0, Ordering::Relaxed);
}

/// Exporta os eventos atuais em `.log` (tmp do usuário); devolve o caminho.
pub fn export() -> std::io::Result<std::path::PathBuf> {
    let path = std::env::temp_dir().join("studio-logs.log");
    let content = entries()
        .into_iter()
        .map(|entry| {
            format!(
                "[{}] [{}] [{}] {} {}",
                entry.timestamp,
                entry.level,
                entry.source,
                entry.message,
                entry.payload.unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, content + "\n")?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_the_buffer_and_keeps_order() {
        // Prefixo único: testes do binary rodam em paralelo no mesmo buffer.
        let prefix = format!("evt-{}-", std::process::id());
        for i in 0..(CAPACITY + 50) {
            super::push(
                "INFO",
                "TEST",
                None,
                format!("{prefix}{i}"),
                String::from("t"),
            );
        }
        let all = entries();
        let mine: Vec<&LogEntry> = all
            .iter()
            .filter(|entry| entry.message.starts_with(&prefix))
            .collect();
        assert_eq!(mine.len(), CAPACITY, "buffer limitado em {CAPACITY}");
        // Ordem preservada: sobrevivem 50..(CAPACITY+50), ascendente.
        assert_eq!(mine[0].message, format!("{prefix}50"));
        assert_eq!(
            mine.last().expect("nonempty").message,
            format!("{prefix}{}", CAPACITY + 49)
        );
    }

    /// PRD 3.14: subsistema/payload/fonte/thread/offset/slot atravessam.
    /// (marcador único: os testes do binary rodam em paralelo e compartilham
    /// o buffer estático — índice absoluto seria flaky.)
    #[test]
    fn tagged_entry_carries_inspector_fields() {
        let marker = format!("falha-inspetor-{}", std::process::id());
        super::push(
            "ERRO",
            "SYSTEMD",
            Some(String::from(r#"{"unit":"llama-server"}"#)),
            marker.clone(),
            String::from("services.rs:120"),
        );
        let all = entries();
        let entry = all
            .iter()
            .rev()
            .find(|entry| entry.message == marker)
            .expect("entrada marcada appended");
        assert_eq!(entry.source, "SYSTEMD");
        assert_eq!(entry.payload.as_deref(), Some(r#"{"unit":"llama-server"}"#));
        assert_eq!(entry.origin, "services.rs:120");
        assert!(entry.thread.starts_with("ThreadId("));
        assert!(entry.backtrace.is_some(), "ERRO captura stack trace");
        assert!(entry.slot > 0);
    }
}
