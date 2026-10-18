//! Painel de visão geral (3.1): agregado honesto do sistema em um relance.

use crate::agents::AgentsState;
use crate::catalog_remote::SharedCatalog;
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::{now_unix_ns, ProbeState};
use crate::models::{ManifestStatus, ModelsState};
use crate::overview::{summarize, OverviewInput, TileHealth};
use crate::services::ServicesPanel;
use crate::state::AppState;
use crate::theme;
use eframe::egui;

/// Dot do cartão pela saúde agregada (tokens do design system).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    services: &mut ServicesPanel,
    agents: &mut AgentsState,
    models: &ModelsState,
    shared: &mut SharedCatalog,
    proof: &str,
    discovery: &DiscoveryState,
) {
    show_band(
        ui, state, services, agents, models, shared, proof, discovery,
    );
    show_dispatch(
        ui, state, services, agents, models, shared, proof, discovery,
    );
    show_sources(
        ui, state, services, agents, models, shared, proof, discovery,
    );
    show_participants(
        ui, state, services, agents, models, shared, proof, discovery,
    );
}

mod overview_band;
mod overview_cards_a;
mod overview_cards_b;
mod overview_dispatch;
mod overview_hero;
mod overview_participants;
mod overview_sources;
mod overview_tele;

use overview_band::show_band;
use overview_cards_a::{agents_card, node_card, services_card};
use overview_cards_b::{catalog_card, inference_card, models_card};
use overview_dispatch::show_dispatch;
use overview_hero::{hero_card, HeroCard};
use overview_participants::show_participants;
use overview_sources::show_sources;
use overview_tele::{health_dot, tele_card, TeleCard};
