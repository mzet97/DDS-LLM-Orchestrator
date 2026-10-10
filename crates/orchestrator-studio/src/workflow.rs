//! Painel "Workflow": cadeia sequencial A→B→C real pelo data space
//! (`wf-run` dentro da GUI — T-890-07, G-25/26).
//!
//! Máquina de estado pura compilável SEM a feature `dds` (testável): o
//! worker é uma closure `FnOnce(Sender<WorkflowEvent>)` despachada para
//! thread de trabalho (padrão `models.rs`/`launch.rs`/T-820-19 — a thread de
//! UI nunca bloqueia). Com `dds`, a view injeta o corredor DDS que replica
//! o `run_seq` do `wf-run`: mesmos prompts congelados
//! (`client::wf_assembly::SEQ_PROMPTS`), mesma montagem, estágios reais
//! reivindicados por agentes.

use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Resultado de um estágio concluído (espelha o `StageRec` do `wf-run`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageOut {
    pub stage: String,
    pub task_id: String,
    pub latency_ms: u64,
    /// Conteúdo truncado para exibição (a íntegra fica no fio/registro).
    pub preview: String,
}

/// Evento do worker para a UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowEvent {
    Stage(StageOut),
    Done {
        total_ms: u64,
        error: Option<String>,
    },
}

/// Configuração do fluxo exibida no formulário.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowConfig {
    pub domain: u32,
    pub entry: String,
    pub model: String,
    pub timeout_ms: u64,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            domain: 170, // laboratório (T-890-03: mesmo default da descoberta)
            entry: String::new(),
            model: String::from("qwen3.5-0.8b"),
            timeout_ms: 120_000,
        }
    }
}

/// Estado do painel (thread-safe via mpsc — mesmo padrão do resto da GUI).
#[derive(Default)]
pub struct WorkflowState {
    pub config: WorkflowConfig,
    pub busy: bool,
    /// Estágios concluídos do fluxo corrente/último.
    pub stages: Vec<StageOut>,
    pub total_ms: Option<u64>,
    pub error: Option<String>,
    pub last_workflow_id: u64,
    rx: Option<Receiver<WorkflowEvent>>,
}

impl WorkflowState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Despacha o fluxo para a thread de trabalho. `runner` recebe o canal e
    /// DEVE enviar `Stage` por estágio concluído e `Done` ao terminar.
    pub fn start<F>(&mut self, runner: F)
    where
        F: FnOnce(u64, Sender<WorkflowEvent>) + Send + 'static,
    {
        if self.busy {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.last_workflow_id += 1;
        let id = self.last_workflow_id;
        self.busy = true;
        self.stages.clear();
        self.total_ms = None;
        self.error = None;
        thread::spawn(move || runner(id, tx));
        self.rx = Some(rx);
    }

    /// Drena os eventos do worker (chamar por frame).
    pub fn poll(&mut self) {
        let mut done = false;
        let Some(rx) = &self.rx else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(WorkflowEvent::Stage(stage)) => self.stages.push(stage),
                Ok(WorkflowEvent::Done { total_ms, error }) => {
                    self.total_ms = Some(total_ms);
                    self.error = error;
                    self.busy = false;
                    done = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // worker morreu sem Done: libera a UI com o erro explícito
                    if self.busy {
                        self.busy = false;
                        self.error = Some(String::from("worker do workflow terminou sem Done"));
                    }
                    done = true;
                    break;
                }
            }
        }
        if done {
            self.rx = None;
        }
    }

    /// Observa um evento (mesma transição do `poll`, testável sem thread).
    pub fn observe(&mut self, event: WorkflowEvent) {
        match event {
            WorkflowEvent::Stage(stage) => self.stages.push(stage),
            WorkflowEvent::Done { total_ms, error } => {
                self.total_ms = Some(total_ms);
                self.error = error;
                self.busy = false;
            }
        }
    }

    /// Trunca conteúdo para a célula da tabela (limiar de exibição).
    #[must_use]
    pub fn preview(content: &str, max_chars: usize) -> String {
        let flattened = content.replace(['\n', '\r'], " ");
        if flattened.chars().count() <= max_chars {
            flattened
        } else {
            let cut: String = flattened.chars().take(max_chars).collect();
            format!("{cut}…")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// O poll roda por frame na GUI; nos testes, drena até o fim com teto
    /// de espera (a thread do worker é concorrente de verdade).
    fn poll_until_idle(state: &mut WorkflowState) {
        for _ in 0..400 {
            state.poll();
            if !state.busy {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("worker não concluiu em 2s");
    }

    #[test]
    fn start_transitions_and_worker_events_land_via_poll() {
        let mut state = WorkflowState::new();
        state.config.entry = String::from("avaliar o serviço");
        assert!(!state.busy);

        state.start(|id, tx| {
            assert_eq!(id, 1, "primeiro fluxo recebe id 1");
            tx.send(WorkflowEvent::Stage(StageOut {
                stage: String::from("A"),
                task_id: String::from("t-a"),
                latency_ms: 500,
                preview: String::from("análise"),
            }))
            .expect("envia estágio");
            tx.send(WorkflowEvent::Done {
                total_ms: 1_600,
                error: None,
            })
            .expect("envia done");
        });
        assert!(state.busy, "run marca busy imediatamente");
        assert!(state.stages.is_empty());

        poll_until_idle(&mut state);
        assert_eq!(state.stages.len(), 1);
        assert_eq!(state.stages[0].stage, "A");
        assert_eq!(state.total_ms, Some(1_600));
        assert!(state.error.is_none());
    }

    #[test]
    fn worker_death_without_done_releases_ui_with_error() {
        let mut state = WorkflowState::new();
        state.start(|_id, _tx| {
            // sai sem enviar nada: canal desconecta
        });
        poll_until_idle(&mut state);
        assert_eq!(
            state.error.as_deref(),
            Some("worker do workflow terminou sem Done")
        );
    }

    #[test]
    fn observe_applies_events_without_worker() {
        let mut state = WorkflowState::new();
        state.busy = true;
        state.observe(WorkflowEvent::Stage(StageOut {
            stage: String::from("C"),
            task_id: String::from("t-c"),
            latency_ms: 700,
            preview: String::from("final"),
        }));
        assert!(state.busy, "Stage não encerra");
        state.observe(WorkflowEvent::Done {
            total_ms: 2_000,
            error: Some(String::from("timeout no estágio C")),
        });
        assert!(!state.busy);
        assert_eq!(state.error.as_deref(), Some("timeout no estágio C"));
    }

    #[test]
    fn preview_flattens_and_truncates() {
        assert_eq!(
            WorkflowState::preview("linha1\nlinha2", 40),
            "linha1 linha2"
        );
        let long = "x".repeat(100);
        let got = WorkflowState::preview(&long, 10);
        assert_eq!(got.chars().count(), 11); // 10 + reticência
        assert!(got.ends_with('…'));
    }

    /// Segundo fluxo enquanto busy é ignorado (não sobrepõe corrida).
    #[test]
    fn start_is_noop_while_busy() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let spawned = Arc::new(AtomicUsize::new(0));
        let mut state = WorkflowState::new();
        let counter = Arc::clone(&spawned);
        state.start(move |_id, tx| {
            counter.fetch_add(1, Ordering::SeqCst);
            let _ = tx.send(WorkflowEvent::Done {
                total_ms: 1,
                error: None,
            });
        });
        // ainda busy (poll não chamado): segundo start NÃO cria worker novo
        state.start(|_id, _tx| panic!("não deveria despachar"));
        poll_until_idle(&mut state);
        assert_eq!(spawned.load(Ordering::SeqCst), 1);
    }
}
