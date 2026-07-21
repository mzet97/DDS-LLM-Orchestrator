//! Regimes de carga do artigo (§6.2) — porte fiel de
//! `benchmarks/experiments/real_workload_driver.py::REGIMES`.
//!
//! `workload_driver.py` (simulado) usa os mesmos λ/burst — as duas fontes
//! convergem para estes valores canônicos.

/// Configuração de um regime de workload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkloadConfig {
    /// Nome canônico ("leve" | "moderada" | "pesada").
    pub name: &'static str,
    /// Taxa base de requests (req/s) — processo de Poisson.
    pub lambda_rps: f64,
    /// Taxa extra durante bursts (req/s; 0 = sem burst).
    pub burst_lambda_rps: f64,
    /// Intervalo entre bursts (s; 0 = sem burst).
    pub burst_interval_s: f64,
    /// Duração de cada burst (s).
    pub burst_duration_s: f64,
    /// Média de tokens do prompt (μ da lognormal = ln(mean)).
    pub prompt_mean_tokens: u32,
    /// `max_tokens` pedido na resposta.
    pub max_tokens_response: u32,
}

/// Leve: λ = 5 req/s, sem burst.
pub const LEVE: WorkloadConfig = WorkloadConfig {
    name: "leve",
    lambda_rps: 5.0,
    burst_lambda_rps: 0.0,
    burst_interval_s: 0.0,
    burst_duration_s: 0.0,
    prompt_mean_tokens: 512,
    max_tokens_response: 50,
};

/// Moderada: λ = 15 req/s, sem burst.
pub const MODERADA: WorkloadConfig = WorkloadConfig {
    name: "moderada",
    lambda_rps: 15.0,
    burst_lambda_rps: 0.0,
    burst_interval_s: 0.0,
    burst_duration_s: 0.0,
    prompt_mean_tokens: 512,
    max_tokens_response: 50,
};

/// Pesada: λ = 30 req/s + bursts de 50 req/s por 0,5 s a cada 10 s.
pub const PESADA: WorkloadConfig = WorkloadConfig {
    name: "pesada",
    lambda_rps: 30.0,
    burst_lambda_rps: 50.0,
    burst_interval_s: 10.0,
    burst_duration_s: 0.5,
    prompt_mean_tokens: 512,
    max_tokens_response: 50,
};

/// Regimes canônicos na ordem do artigo.
pub const REGIMES: [WorkloadConfig; 3] = [LEVE, MODERADA, PESADA];

/// Busca regime pelo nome (None se desconhecido — como o `ValueError` do Python).
pub fn regime(name: &str) -> Option<WorkloadConfig> {
    REGIMES.iter().copied().find(|r| r.name == name)
}
