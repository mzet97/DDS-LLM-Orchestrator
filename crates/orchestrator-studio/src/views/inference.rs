//! Painel 3.3 Inferência & Chat de Engenharia: fiel ao mockup Stitch —
//! cabeçalho com planos DDS/HTTP, descoberta ServerStatus (linha por
//! servidor), endpoint HTTP + model probe, parâmetros de geração, transcript
//! com stats reais e composer com Ctrl+Enter.
//!
//! Só dados reais: sem GUID de servidor (o fio não tem), sem build/CUDA do
//! alvo (o `/v1/models` não anuncia), sem VRAM/slots inventados e sem
//! denominador de contexto (o servidor não informa `n_ctx`). O QoS do
//! `ServerStatus` segue o CÓDIGO (`qos.rs`: BestEffort + Volatile +
//! KeepLast(1)) — o mockup diz "TransientLocal · Reliable", o que contradiz
//! o contrato e a si mesmo (durabilidades alternativas).

use crate::discovery::DiscoveryState;
use crate::inference::{InferenceState, Role};
use crate::kit;
use crate::theme;
use eframe::egui;

/// Moldura dos cards da 3.3 (mesmo idioma da 3.2/3.10).
pub fn show(ui: &mut egui::Ui, infer: &mut InferenceState, discovery: &DiscoveryState) {
    infer.poll();
    if infer.busy {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
    }

    show_header(ui, infer, discovery);
    show_discovery(ui, infer, discovery);
    show_endpoint(ui, infer, discovery);
    show_transcript(ui, infer, discovery);
    show_generation(ui, infer, discovery);
    show_composer(ui, infer, discovery);
}

mod inference_composer;
mod inference_discovery;
mod inference_endpoint;
mod inference_generation;
mod inference_header;
mod inference_helpers;
mod inference_transcript;

use inference_composer::show_composer;
use inference_discovery::show_discovery;
use inference_endpoint::show_endpoint;
use inference_generation::show_generation;
use inference_header::show_header;
use inference_helpers::{
    card_frame, card_slider, card_title, hhmmss_utc, session_tokens, temp_caption, temp_mode,
};
use inference_transcript::show_transcript;

#[cfg(test)]
mod tests;
