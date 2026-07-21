//! Cenários canônicos E1–E5 / OP1–OP4 da dissertação.
//!
//! Cada entrada cita a fonte Python de onde os parâmetros foram extraídos
//! (campo `source`). Parâmetros não verificados no código-fonte NÃO são
//! inventados — ficam como follow-up documentado em `notes`.

use crate::regimes::{self, WorkloadConfig};

/// Padrão de geração de carga do cenário.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkloadPattern {
    /// Open-loop: chegadas Poisson segundo um regime (E4/E5; OP2).
    Open { regime: WorkloadConfig },
    /// Closed-loop: N workers concorrentes, cada um sequencial
    /// request→resposta (OP1/E2 — vazão sob concorrência limitada).
    Closed { concurrency: &'static [u32] },
    /// Carga de fundo NORMAL + injeções HIGH periódicas (E3/OP4).
    /// Fonte: `benchmarks/E3_priority.py` (carga 5 req/s, duração 100 s).
    Priority {
        background_rps: f64,
        n_injections: u32,
    },
    /// Streaming: mede TTFT/ITL/e2e por chunk (E5).
    Streaming { regime: WorkloadConfig },
}

/// Definição de um cenário de benchmark.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// Identificador canônico ("E1".."E5", "OP1".."OP4").
    pub id: &'static str,
    /// Nome curto (como na tabela do README da qualificação).
    pub name: &'static str,
    /// Fonte Python dos parâmetros (rastreabilidade).
    pub source: &'static str,
    /// Padrão de carga.
    pub pattern: WorkloadPattern,
    /// Duração default da medição (s), excluindo warmup.
    pub duration_s: f64,
    /// Warmup descartado antes de medir (s).
    pub warmup_s: f64,
    /// Repetições por condição (o artigo usa 30; a qualificação N=1000).
    pub replications: u32,
    /// Notas de porte / follow-ups honestos.
    pub notes: &'static str,
}

impl Scenario {
    /// Validação estrutural (usada nos testes e no CLI).
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.name.is_empty() {
            return Err("id/name vazios".into());
        }
        if self.duration_s <= 0.0 {
            return Err(format!("{}: duration_s deve ser > 0", self.id));
        }
        if self.replications == 0 {
            return Err(format!("{}: replications deve ser > 0", self.id));
        }
        match &self.pattern {
            WorkloadPattern::Open { regime } | WorkloadPattern::Streaming { regime } => {
                if regime.lambda_rps <= 0.0 {
                    return Err(format!("{}: lambda_rps deve ser > 0", self.id));
                }
            }
            WorkloadPattern::Closed { concurrency } => {
                if concurrency.is_empty() || concurrency.contains(&0) {
                    return Err(format!("{}: concurrency inválida", self.id));
                }
            }
            WorkloadPattern::Priority {
                background_rps,
                n_injections,
            } => {
                if *background_rps <= 0.0 || *n_injections == 0 {
                    return Err(format!("{}: priority params inválidos", self.id));
                }
            }
        }
        Ok(())
    }
}

/// Registry canônico — 9 cenários.
pub fn registry() -> &'static [Scenario] {
    &REGISTRY
}

/// Busca por id (case-insensitive).
pub fn get(id: &str) -> Option<&'static Scenario> {
    REGISTRY.iter().find(|s| s.id.eq_ignore_ascii_case(id))
}

static REGISTRY: [Scenario; 9] = [
    Scenario {
        id: "E1",
        name: "Decomposição de Latência",
        source: "benchmarks/qualificacao/scripts/run_t_decomp.py",
        pattern: WorkloadPattern::Open {
            regime: regimes::LEVE,
        },
        duration_s: 300.0,
        warmup_s: 30.0,
        replications: 30,
        notes: "Spans por camada vêm dos campos t_*_ns do Task (IDL). Spans não \
                observáveis saem como ausentes (measurement_type=inferred no Python), \
                nunca fabricados.",
    },
    Scenario {
        id: "E2",
        name: "Vazão sob Concorrência",
        source: "benchmarks/qualificacao/configs/experiment_n1000.yaml (concurrency: [10, 50])",
        pattern: WorkloadPattern::Closed {
            concurrency: &[10, 50],
        },
        duration_s: 300.0,
        warmup_s: 30.0,
        replications: 1000,
        notes: "Qualificação roda N=1000 replicações por cenário com seed 42.",
    },
    Scenario {
        id: "E3",
        name: "Prioridade",
        source: "benchmarks/E3_priority.py (carga=5 req/s NORMAL, injeções HIGH, duração=100 s)",
        pattern: WorkloadPattern::Priority {
            background_rps: 5.0,
            n_injections: 20,
        },
        duration_s: 100.0,
        warmup_s: 0.0,
        replications: 30,
        notes: "Métrica: mediana(NORMAL) − mediana(HIGH) — vantagem esperada > 0.",
    },
    Scenario {
        id: "E4",
        name: "Seleção Dinâmica de QoS (5 braços)",
        source: "benchmarks/experiments/{workload_driver,real_workload_driver}.py (artigo §6.2)",
        pattern: WorkloadPattern::Open {
            regime: regimes::MODERADA,
        },
        duration_s: 300.0,
        warmup_s: 30.0,
        replications: 30,
        notes: "Cada braço (static/zadeh/fcm/fcm-dhl/nfcm + baselines) roda os 3 \
                regimes (leve/moderada/pesada); comparativo por JSONL por braço/cenário.",
    },
    Scenario {
        id: "E5",
        name: "TTFT e Streaming",
        source: "benchmarks/qualificacao/scripts/run_e5_ttft.py",
        pattern: WorkloadPattern::Streaming {
            regime: regimes::LEVE,
        },
        duration_s: 300.0,
        warmup_s: 30.0,
        replications: 30,
        notes: "TTFT = 1º chunk − submit; ITL = deltas entre chunks; e2e até is_final.",
    },
    Scenario {
        id: "OP1",
        name: "Vazão Básica (reduzido)",
        source: "benchmarks/qualificacao/scripts/run_op1_reduced.py (concorrência 1, 10, 25, 50)",
        pattern: WorkloadPattern::Closed {
            concurrency: &[1, 10, 25, 50],
        },
        duration_s: 120.0,
        warmup_s: 10.0,
        replications: 30,
        notes: "NÃO calcula goodput-SLO completo (aviso do próprio script Python).",
    },
    Scenario {
        id: "OP2",
        name: "Utilização e Fairness",
        source: "benchmarks/qualificacao/scripts/run_op2_fairness.py",
        pattern: WorkloadPattern::Open {
            regime: regimes::MODERADA,
        },
        duration_s: 300.0,
        warmup_s: 30.0,
        replications: 30,
        notes: "Distribuição de tasks por agente (índice de Jain calculado na análise \
                Python, não no driver).",
    },
    Scenario {
        id: "OP3",
        name: "Recuperação após Falha",
        source: "benchmarks/qualificacao/configs/experiment_op3_failover_linux.yaml (seed 25010)",
        pattern: WorkloadPattern::Open {
            regime: regimes::LEVE,
        },
        duration_s: 300.0,
        warmup_s: 30.0,
        replications: 30,
        notes: "Prompt long_recovery (256 tokens out). A falha é injetada fora do \
                driver (kill do agente) — o driver mede a recuperação.",
    },
    Scenario {
        id: "OP4",
        name: "Priorização sob Carga",
        source: "benchmarks/qualificacao/scripts/run_op4_priority.py",
        pattern: WorkloadPattern::Priority {
            background_rps: 15.0,
            n_injections: 30,
        },
        duration_s: 300.0,
        warmup_s: 30.0,
        replications: 30,
        notes: "Igual E3 mas sob carga moderada de fundo (15 req/s).",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_tem_os_9_cenarios() {
        assert_eq!(registry().len(), 9);
        for id in ["E1", "E2", "E3", "E4", "E5", "OP1", "OP2", "OP3", "OP4"] {
            assert!(get(id).is_some(), "cenário {id} ausente");
        }
    }

    #[test]
    fn ids_unicos() {
        let mut ids: Vec<_> = registry().iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), registry().len());
    }

    #[test]
    fn todos_validam() {
        for s in registry() {
            s.validate()
                .unwrap_or_else(|e| panic!("{} inválido: {e}", s.id));
        }
    }

    #[test]
    fn e3_espelha_o_script_python() {
        let e3 = get("E3").unwrap();
        match &e3.pattern {
            WorkloadPattern::Priority {
                background_rps,
                n_injections,
            } => {
                assert_eq!(*background_rps, 5.0); // --carga default 5
                assert!(*n_injections > 0);
            }
            other => panic!("E3 deveria ser Priority, é {other:?}"),
        }
        assert_eq!(e3.duration_s, 100.0); // --duracao default 100
    }

    #[test]
    fn op1_espelha_concorrencias_do_script() {
        let op1 = get("OP1").unwrap();
        match &op1.pattern {
            WorkloadPattern::Closed { concurrency } => {
                assert_eq!(*concurrency, [1, 10, 25, 50]);
            }
            other => panic!("OP1 deveria ser Closed, é {other:?}"),
        }
    }

    #[test]
    fn validate_rejeita_duration_zero() {
        let mut s = get("E1").unwrap().clone();
        s.duration_s = 0.0;
        assert!(s.validate().is_err());
    }
}
