//! QosDecider trait — interface comum para decisores de QoS (REQ-501, REQ-504).
//!
//! Todos os 5 braços implementam esta trait:
//! - Static: perfil fixo (controle)
//! - Zadeh: score ponderado linear
//! - FCM: Fuzzy Cognitive Map com pesos fixos
//! - FCM+DHL: FCM com aprendizado Hebbiano
//! - NFCM: Neuro-Fuzzy Cognitive Map (já implementado)

use crate::QoSProfile;

/// Métricas de entrada para decisão de QoS.
#[derive(Debug, Clone)]
pub struct QoSMetrics {
    pub urgency: f64,
    pub deadline_pressure: f64,
    pub recent_latency: f64,
    pub agent_load: f64,
    pub error_rate: f64,
    pub historical_confidence: f64,
    pub estimated_complexity: f64,
    pub streaming_need: f64,
}

impl Default for QoSMetrics {
    fn default() -> Self {
        Self {
            urgency: 0.5,
            deadline_pressure: 0.5,
            recent_latency: 0.5,
            agent_load: 0.5,
            error_rate: 0.1,
            historical_confidence: 0.5,
            estimated_complexity: 0.5,
            streaming_need: 0.5,
        }
    }
}

/// Resultado da decisão de QoS.
#[derive(Debug, Clone)]
pub struct QoSDecision {
    pub profile: QoSProfile,
    pub confidence: f64,
    pub explanation: String,
    /// `false` apenas quando um decisor iterativo (FCM/NFCM) atingiu `T_max`
    /// sem convergir — o control loop usa isto para a política de fallback do
    /// artigo (§4.3: "mantém o perfil atual"). Decisores não-iterativos
    /// (static/zadeh/regras/bandits) são trivialmente `true`.
    pub converged: bool,
    /// Score do segundo colocado (0.0 quando o decisor não produz ranking) —
    /// insumo do `StabilityController` (histerese exige margem
    /// `confidence - runner_up > m` antes de trocar de perfil).
    pub runner_up: f64,
}

/// Trait comum para decisores de QoS.
pub trait QosDecider: Send + Sync {
    /// Decide qual perfil QoS usar com base nas métricas.
    fn decide(&self, metrics: &QoSMetrics) -> QoSDecision;

    /// Nome do decisor (para logging/trace).
    fn name(&self) -> &str;
}

/// Decisor estático — sempre retorna o mesmo perfil (REQ-501).
pub struct StaticDecider {
    profile: QoSProfile,
}

impl StaticDecider {
    pub fn new(profile: QoSProfile) -> Self {
        Self { profile }
    }
}

impl QosDecider for StaticDecider {
    fn decide(&self, _metrics: &QoSMetrics) -> QoSDecision {
        QoSDecision {
            profile: self.profile.clone(),
            confidence: 1.0,
            explanation: format!("static: always {:?}", self.profile),
            converged: true,
            runner_up: 0.0,
        }
    }

    fn name(&self) -> &str {
        "static"
    }
}
