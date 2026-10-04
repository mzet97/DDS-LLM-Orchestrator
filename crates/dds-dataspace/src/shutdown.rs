//! Flag de shutdown compartilhada (T-820-05/P1-7 — revisão 2026-10-04).
//!
//! O `DataSpaceApi::shutdown`/`Drop` do `DataSpace` real derruba o
//! participant/waitset via RAII, mas as streams sobreviviam penduradas para
//! sempre num `Notified` que nunca mais dispara (o driver do waitset é
//! abortado no `Drop` do `SharedWaitSet`). Esta flag é o mecanismo barato que
//! permite aos geradores de stream terminarem graciosamente: o `Drop` do
//! `SharedWaitSet` a marca e acorda os registros pendentes; cada stream
//! checa `Registration::is_shutdown()` antes/depois de esperar e encerra o
//! stream (`None`) em vez de pendurar o consumidor.
//!
//! Módulo SEM dependência de DDS de propósito: a semântica da flag é
//! testável sem `--features dds` (ver `tests` abaixo), e o restante do
//! mecanismo (acordar registros no `Drop`) é exercitado pelos testes de
//! integração cfg(dds) de `tests/shared_waitset.rs`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Flag one-shot compartilhada entre o dono do recurso (que a marca no
/// teardown) e os consumidores (streams), que a consultam via clone barato
/// (`Arc<AtomicBool>`).
///
/// `Clone` compartilha o MESMO estado (é um handle, não uma cópia) — é assim
/// que cada `Registration` observa o shutdown do `SharedWaitSet` que o criou.
#[derive(Debug, Clone, Default)]
pub struct ShutdownFlag(Arc<AtomicBool>);

impl ShutdownFlag {
    /// Cria a flag desmarcada.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Marca o shutdown (idempotente; chamado no teardown do dono).
    pub fn set(&self) {
        // SeqCst: o `set` precisa ser visível a QUALQUER leitura posterior em
        // outra thread antes do `notify_waiters` correspondente ser observado
        // — é a barreira que garante que um consumidor acordado pela
        // notificação enxerga a flag marcada.
        self.0.store(true, Ordering::SeqCst);
    }

    /// `true` quando o dono do recurso já encerrou.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_new_start_unset() {
        assert!(!ShutdownFlag::new().is_set());
        assert!(!ShutdownFlag::default().is_set());
    }

    #[test]
    fn clones_share_state() {
        let flag = ShutdownFlag::new();
        let clone = flag.clone();
        assert!(!clone.is_set());
        flag.set();
        // Clone observa a marcação do original (mesmo Arc).
        assert!(clone.is_set());
        assert!(flag.is_set());
    }

    #[test]
    fn set_is_idempotent_and_visible_across_threads() {
        let flag = ShutdownFlag::new();
        flag.set();
        flag.set();
        assert!(flag.is_set());
        let handle = std::thread::spawn(move || flag.is_set());
        assert!(handle.join().expect("thread ok"));
    }
}
