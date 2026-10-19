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
mod tests;
