//! Parte de `studio_log` (extraída para manter os arquivos revisáveis).

use super::*;

#[derive(Debug)]
pub struct LogsPanel {
    pub filter_level: Option<&'static str>,
    pub search: String,
    pub auto_scroll: bool,
    /// Espelho stderr ligado (comanda o `MIRROR_STDERR` global).
    pub mirror: bool,
    /// Lista congelada (pausa de DISPLAY — o buffer segue absorvendo).
    pub paused: bool,
    pub frozen: Vec<LogEntry>,
    /// Slot da entrada selecionada no inspetor lateral (3.14).
    pub selected: Option<u64>,
}

impl Default for LogsPanel {
    fn default() -> Self {
        Self {
            filter_level: None,
            search: String::new(),
            auto_scroll: true,
            mirror: true,
            paused: false,
            frozen: Vec::new(),
            selected: None,
        }
    }
}

impl LogsPanel {
    /// Padrão de console: cauda travada no novo + espelho ligado.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Espelho stderr ligado?
#[must_use]
pub fn mirror() -> bool {
    MIRROR_STDERR.load(Ordering::Relaxed)
}

/// Liga/desliga o espelho stderr (checkbox da 3.14).
pub fn set_mirror(on: bool) {
    MIRROR_STDERR.store(on, Ordering::Relaxed);
}

/// Eventos saídos do buffer (evicção FIFO + limpezas manuais): slots
/// emitidos menos retidos.
#[must_use]
pub fn evicted() -> u64 {
    let pushed = NEXT_SLOT.load(Ordering::Relaxed).saturating_sub(1);
    let kept = entries().len() as u64;
    pushed.saturating_sub(kept)
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
