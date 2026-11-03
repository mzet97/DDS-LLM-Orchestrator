//! Subir inferência local: plano legível → aplicar → comprovar (§9.3, P2).
//!
//! Escopo honesto de máquina única: o plano atua `start` numa unidade
//! própria já conhecida do `studio-node` (criar unidade nova com parâmetros
//! arbitrários não existe no nó — sem simular). A comprovação é geração
//! real no `llama-server`. Tudo pesado roda em thread dedicada com `poll`.

use std::sync::mpsc;

use thiserror::Error;

use crate::inference::{chat_completion, list_models, ChatRequest, Message, Role};
use crate::services::{actuate, fresh_operation_id, list_services};

/// Dispositivo pedido no plano (informativo: o binário efetivo é o da unidade).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Cpu,
    Gpu,
}

/// Plano de subida: só referencia unidade conhecida + prova observável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub node_url: String,
    pub llama_url: String,
    pub service: String,
    pub device: Device,
    pub ctx_tokens: u32,
    pub slots: u32,
    pub dds_route: String,
    pub proof_prompt: String,
}

/// Erros do assistente (validação local antes de qualquer efeito).
#[derive(Debug, Error)]
pub enum LaunchError {
    /// Campo inválido ou unidade desconhecida: nada foi atuado.
    #[error("plano invalido: {detail}")]
    Invalid { detail: String },
}

impl LaunchPlan {
    /// Padrões honestos: nó e llama locais, prova mínima.
    #[must_use]
    pub fn new() -> Self {
        Self {
            node_url: String::from("http://127.0.0.1:4317"),
            llama_url: String::from("http://127.0.0.1:8082"),
            service: String::new(),
            device: Device::Gpu,
            ctx_tokens: 4096,
            slots: 1,
            dds_route: String::from("LLM.InferenceRequest (domínio 0)"),
            proof_prompt: String::from("Responda apenas: INFERENCIA_OK"),
        }
    }

    /// Valida sem efeito: unidade precisa existir no nó; números, ser úteis.
    pub fn validate(&self, known_services: &[String]) -> Result<(), LaunchError> {
        let invalid = |detail: &str| LaunchError::Invalid {
            detail: String::from(detail),
        };
        if self.service.trim().is_empty() {
            return Err(invalid("escolha a unidade na lista lida do nó"));
        }
        if !known_services.iter().any(|name| name == &self.service) {
            return Err(invalid(
                "unidade fora da lista lida do nó — releia os serviços",
            ));
        }
        if self.ctx_tokens == 0 || self.slots == 0 {
            return Err(invalid("contexto e slots precisam ser maiores que zero"));
        }
        if self.proof_prompt.trim().is_empty() {
            return Err(invalid("a prova precisa de um prompt"));
        }
        Ok(())
    }

    /// Prévia legível: as 5 etapas canônicas (PRD 3.4) antes de qualquer efeito.
    #[must_use]
    pub fn preview(&self) -> String {
        let device = match self.device {
            Device::Cpu => "CPU",
            Device::Gpu => "GPU (exige build com o backend real)",
        };
        format!(
            "1. GET {}/services — ① verificar unidade & binário (hash .gguf: tela 3.7)\n\
             2. POST {}/services/{}/start — ② spawn do processo (systemd)\n\
             3. aguardar active=true (até 60 s) — ③ processo vivo\n\
             4. GET {}/v1/models — ④ modelo & KV cache alocados\n\
             5. prova: POST /v1/chat/completions (temp 0, 32 tokens) — ⑤ geração + ServerStatus\n\
             dispositivo pedido: {device} · contexto: {} · slots: {}\n\
             rota DDS pretendida: {}\n\
             NADA é criado no nó: só start de unidade própria existente.",
            self.node_url,
            self.node_url,
            self.service,
            self.llama_url,
            self.ctx_tokens,
            self.slots,
            self.dds_route
        )
    }
}

impl Default for LaunchPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// Resultado de uma etapa da aplicação (para o painel, passo a passo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    pub step: &'static str,
    pub ok: bool,
    pub detail: String,
    /// Duração medida da etapa (tela 3.4 — wizard com tempos reais).
    pub duration_ms: u64,
}

enum LaunchMsg {
    Step(StepResult),
    Finished,
}

/// Estado do assistente: formulário + execução em segundo plano.
pub struct LaunchState {
    pub plan: LaunchPlan,
    pub running: bool,
    pub steps: Vec<StepResult>,
    pub error: String,
    receiver: Option<mpsc::Receiver<LaunchMsg>>,
}

impl LaunchState {
    /// Assistente fechado no formulário vazio, sem efeito.
    #[must_use]
    pub fn new() -> Self {
        Self {
            plan: LaunchPlan::new(),
            running: false,
            steps: Vec::new(),
            error: String::new(),
            receiver: None,
        }
    }

    /// Valida e dispara `runner` em thread; a UI segue livre (`poll` drena).
    pub fn start_with(
        &mut self,
        known_services: &[String],
        runner: impl FnOnce(LaunchPlan) -> Vec<StepResult> + Send + 'static,
    ) {
        if self.running {
            return;
        }
        match self.plan.clone().validate(known_services) {
            Ok(()) => {
                self.error.clear();
                self.steps.clear();
                let plan = self.plan.clone();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    for step in runner(plan) {
                        let failed = !step.ok;
                        let _ = tx.send(LaunchMsg::Step(step));
                        if failed {
                            break;
                        }
                    }
                    let _ = tx.send(LaunchMsg::Finished);
                });
                self.receiver = Some(rx);
                self.running = true;
            }
            Err(err) => {
                self.error = err.to_string();
            }
        }
    }

    /// Caminho real: atua o nó, espera, lista modelos e prova geração.
    pub fn start(&mut self, known_services: &[String]) {
        self.start_with(known_services, run_launch);
    }

    /// Drena as etapas; chamar a cada frame enquanto `running`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    LaunchMsg::Step(step) => {
                        if !step.ok && self.error.is_empty() {
                            self.error = format!("{}: {}", step.step, step.detail);
                        }
                        self.steps.push(step);
                    }
                    LaunchMsg::Finished => {
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.receiver = None;
            self.running = false;
        }
    }

    /// `true` quando todas as etapas executaram com sucesso.
    #[must_use]
    pub fn proved(&self) -> bool {
        !self.running && !self.steps.is_empty() && self.steps.iter().all(|step| step.ok)
    }
}

impl Default for LaunchState {
    fn default() -> Self {
        Self::new()
    }
}

/// Executa o plano real com cronômetro por etapa — as 5 etapas canônicas do
/// PRD 3.4, cada uma amarrada a uma operação REAL do nó/servidor:
/// ① verificar unidade & binário (GET /services — o hash .gguf é a tela 3.7)
/// ② spawn do processo (POST /services/{u}/start — systemd cria o processo)
/// ③ aguardar processo vivo (active=true em GET /services)
/// ④ modelo & KV cache (GET /v1/models — modelo alocado pela unidade)
/// ⑤ prova de geração (chat real; a publicação ServerStatus é conferida
///    ao vivo pela descoberta na view).
mod launch_exec;

pub(crate) use launch_exec::run_launch;

#[cfg(test)]
mod tests;
