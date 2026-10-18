//! Parte de `views::workflow` (extraída para manter os arquivos revisáveis).

#[cfg(feature = "dds")]
use super::*;

/// Estado visual de um estágio (derivação honesta do estado real).
#[cfg(feature = "dds")]
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "dds")]
/// Nome canônico do estágio (espelho do PNG): a chave interna ("A"/"B"/"C"
/// do `wf_assembly`) ganha o rótulo do processo de negócio.
#[cfg(feature = "dds")]
/// Inteiro com separador de milhar (espelho do PNG: `2,080 ms`).
#[cfg(any(test, feature = "dds"))]
#[cfg(feature = "dds")]
pub(super) enum StageVisual {
    Pendente,
    Executando,
    Concluido,
    Falhou,
}

#[cfg(feature = "dds")]
pub(super) fn stage_of<'a>(stages: &'a [StageOut], name: &str) -> Option<&'a StageOut> {
    stages.iter().find(|stage| stage.stage == name)
}

#[cfg(feature = "dds")]
pub(super) fn stage_display(name: &str) -> String {
    match name {
        "A" => String::from("A Triagem & Fatos"),
        "B" => String::from("B Síntese & Resolução LLM"),
        "C" => String::from("C Validação & Formatação"),
        other => other.to_owned(),
    }
}

pub(super) fn fmt_int(n: u64) -> String {
    let digits = n.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

#[cfg(feature = "dds")]
pub(super) fn stage_visual(state: &WorkflowState, name: &str) -> StageVisual {
    if stage_of(&state.stages, name).is_some() {
        return StageVisual::Concluido;
    }
    if let Some(error) = &state.error {
        if error.contains(&format!("estágio {name}")) {
            return StageVisual::Falhou;
        }
    }
    if state.busy {
        // Primeiro estágio sem card = o que está executando agora.
        let first_missing = ["A", "B", "C"]
            .iter()
            .find(|candidate| stage_of(&state.stages, candidate).is_none());
        if first_missing == Some(&name) {
            return StageVisual::Executando;
        }
    }
    StageVisual::Pendente
}
