//! Registro de eventos da GUI (pedido do autor 2026-10-06): analisar o
//! comportamento do Studio — descoberta, auto-carga, observação DDS —
//! sem depender só do stderr. Cada evento vai para o stderr E para um
//! buffer circular em memória exibido no painel "Logs".
//!
//! Deliberadamente simples (sem tracing/subscriber): os sítios de log são
//! poucos e o objetivo é depuração operacional, não telemetria.

use std::collections::VecDeque;
use std::sync::Mutex;

/// Capacidade do buffer (eventos mais antigos são descartados).
const CAPACITY: usize = 500;

/// Um evento registrado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// Instante wall-clock legível (RFC3339 curto, hora local).
    pub timestamp: String,
    pub level: &'static str,
    pub message: String,
}

static ENTRIES: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());

fn push(level: &'static str, message: String) {
    let timestamp = chrono_now();
    // Sob teste: sem poluir o stderr do cargo com centenas de eventos.
    if !cfg!(test) {
        eprintln!("studio[{level}] {timestamp} {message}");
    }
    if let Ok(mut entries) = ENTRIES.lock() {
        if entries.len() == CAPACITY {
            entries.pop_front();
        }
        entries.push_back(LogEntry {
            timestamp,
            level,
            message,
        });
    }
}

/// Hora local compacta (sem depender de chrono: HH:MM:SS do sistema).
fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    format!("{h:02}:{m:02}:{s:02}")
}

/// Registra um evento informativo.
pub fn info(message: impl Into<String>) {
    push("INFO", message.into());
}

/// Registra um aviso (falha recuperável).
pub fn warn(message: impl Into<String>) {
    push("WARN", message.into());
}

/// Registra um erro.
pub fn error(message: impl Into<String>) {
    push("ERRO", message.into());
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
/// busca por substring e auto-scroll.
#[derive(Debug, Default)]
pub struct LogsPanel {
    pub filter_level: Option<&'static str>,
    pub search: String,
    pub auto_scroll: bool,
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
}

/// Exporta os eventos atuais em `.log` (tmp do usuário); devolve o caminho.
pub fn export() -> std::io::Result<std::path::PathBuf> {
    let path = std::env::temp_dir().join("studio-logs.log");
    let content = entries()
        .into_iter()
        .map(|entry| format!("[{}] {} {}", entry.timestamp, entry.level, entry.message))
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
        for i in 0..(CAPACITY + 50) {
            super::push("INFO", format!("evento {i}"));
        }
        let entries = entries();
        assert_eq!(entries.len(), CAPACITY, "buffer limitado em {CAPACITY}");
        // Ordem preservada: o primeiro é o mais antigo sobrevivente (50) e o
        // último é o mais recente (CAPACITY + 49).
        assert_eq!(entries[0].message, "evento 50");
        assert_eq!(
            entries.last().expect("nonempty").message,
            format!("evento {}", CAPACITY + 49)
        );
    }
}
