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

    /// Prévia legível: mostra o que será feito antes de qualquer efeito.
    #[must_use]
    pub fn preview(&self) -> String {
        let device = match self.device {
            Device::Cpu => "CPU",
            Device::Gpu => "GPU (exige build com o backend real)",
        };
        format!(
            "1. POST {}/services/{}/start (operação idempotente nova)\n\
             2. aguardar active=true em GET /services (até 60s)\n\
             3. GET {}/v1/models e usar o primeiro modelo anunciado\n\
             4. prova: POST /v1/chat/completions, temp 0, 32 tokens\n\
             dispositivo pedido: {device} · contexto: {} · slots: {}\n\
             rota DDS pretendida: {}\n\
             NADA é criado no nó: só start de unidade própria existente.",
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

/// Execução real do plano, passo a passo, fora da thread de UI.
fn run_launch(plan: LaunchPlan) -> Vec<StepResult> {
    let mut steps = Vec::with_capacity(4);
    let fail = |step: &'static str, detail: String| StepResult {
        step,
        ok: false,
        detail,
    };
    let done = |step: &'static str, detail: String| StepResult {
        step,
        ok: true,
        detail,
    };
    match actuate(
        &plan.node_url,
        &plan.service,
        true,
        &fresh_operation_id(&plan.service),
    ) {
        Ok(out) if out.active || !out.acted => steps.push(done(
            "atuar start",
            format!("wanted=true active={} acted={}", out.active, out.acted),
        )),
        Ok(out) => steps.push(done(
            "atuar start",
            format!("registrado; active={} (aguardando)", out.active),
        )),
        Err(err) => {
            steps.push(fail("atuar start", err.to_string()));
            return steps;
        }
    }
    let mut active = false;
    for _ in 0..60 {
        match list_services(&plan.node_url) {
            Ok(list)
                if list
                    .iter()
                    .any(|item| item.service == plan.service && item.active) =>
            {
                active = true;
                break;
            }
            Ok(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
            Err(err) => {
                steps.push(fail("aguardar ativo", err.to_string()));
                return steps;
            }
        }
    }
    if !active {
        steps.push(fail(
            "aguardar ativo",
            String::from("unidade não ficou ativa em 60s"),
        ));
        return steps;
    }
    steps.push(done("aguardar ativo", String::from("active=true")));
    let model = match list_models(&plan.llama_url) {
        Ok(models) if !models.is_empty() => {
            let id = models[0].id.clone();
            steps.push(done("listar modelos", format!("primeiro anunciado: {id}")));
            id
        }
        Ok(_) => {
            steps.push(fail(
                "listar modelos",
                String::from("servidor sem modelos anunciados"),
            ));
            return steps;
        }
        Err(err) => {
            steps.push(fail("listar modelos", err.to_string()));
            return steps;
        }
    };
    match chat_completion(
        &plan.llama_url,
        &ChatRequest {
            model,
            messages: vec![Message {
                role: Role::User,
                content: plan.proof_prompt.clone(),
            }],
            temperature: 0.0,
            max_tokens: 32,
        },
    ) {
        Ok(reply) => steps.push(done("prova de geração", reply)),
        Err(err) => steps.push(fail("prova de geração", err.to_string())),
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Vec<String> {
        vec![String::from("llama-server")]
    }

    #[test]
    fn empty_service_never_actuates() {
        let plan = LaunchPlan::new();

        let err = plan.validate(&known()).expect_err("vazio deve falhar");

        assert!(matches!(err, LaunchError::Invalid { .. }));
    }

    #[test]
    fn unknown_service_is_rejected_before_any_effect() {
        let mut plan = LaunchPlan::new();
        plan.service = String::from("unidade-fantasma");

        let err = plan
            .validate(&known())
            .expect_err("desconhecida deve falhar");

        assert!(err.to_string().contains("lista lida do nó"));
    }

    #[test]
    fn preview_states_everything_before_effect() {
        let mut plan = LaunchPlan::new();
        plan.service = String::from("llama-server");

        let preview = plan.preview();

        assert!(preview.contains("POST"));
        assert!(preview.contains("llama-server"));
        assert!(preview.contains("NADA é criado"));
    }

    #[test]
    fn stub_runner_drives_steps_through_poll() {
        let mut state = LaunchState::new();
        state.plan.service = String::from("llama-server");
        state.start_with(&known(), |_| {
            vec![
                StepResult {
                    step: "atuar start",
                    ok: true,
                    detail: String::from("ok"),
                },
                StepResult {
                    step: "prova de geração",
                    ok: true,
                    detail: String::from("INFERENCIA_OK"),
                },
            ]
        });

        for _ in 0..1000 {
            state.poll();
            if !state.running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(state.proved());
        assert_eq!(state.steps.len(), 2);
    }

    #[test]
    fn failing_step_surfaces_first_error() {
        let mut state = LaunchState::new();
        state.plan.service = String::from("llama-server");
        state.start_with(&known(), |_| {
            vec![StepResult {
                step: "atuar start",
                ok: false,
                detail: String::from("conexão recusada"),
            }]
        });

        for _ in 0..1000 {
            state.poll();
            if !state.running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(!state.proved());
        assert!(state.error.contains("conexão recusada"));
    }
}
