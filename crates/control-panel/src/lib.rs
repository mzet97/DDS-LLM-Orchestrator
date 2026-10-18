//! Painel de controle Rust (fase 910) — migração de `src/control_panel`.
//!
//! Supervisor local de processos dos binários Rust atuais, catálogo/presets,
//! API HTTP (axum), hardware, inventário Ansible com redação, test runner e
//! construção de mensagens de chat. Ver `specs/910-control-panel/`.

pub mod ansible;
pub mod ansible_runs;
pub mod catalog;
pub mod chat;
pub mod config;
pub mod hardware;
pub mod host_status;
pub mod inventory;
pub mod presets;
pub mod process;
pub mod routes_ops;
pub mod routes_services;
pub mod routes_vms;
pub mod server;
pub mod settings;
pub mod supervisor;
pub mod test_runner;
