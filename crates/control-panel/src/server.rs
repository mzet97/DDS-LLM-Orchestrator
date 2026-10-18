//! API HTTP do supervisor (axum), espelhando os endpoints Flask de
//! `supervisor.py:216-301` e as rotas web de dashboard/hardware.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::routes_ops::*;
use crate::routes_services::*;
use crate::routes_vms::*;
use crate::supervisor::Supervisor;

/// Estado compartilhado da API.
pub struct AppState {
    pub supervisor: Arc<Supervisor>,
    pub ansible: Arc<crate::ansible::AnsibleRunner>,
    pub tests: Arc<crate::test_runner::TestRunner>,
    pub host_status: Arc<crate::host_status::HostStatusCache>,
}

impl AppState {
    pub fn new(supervisor: Arc<Supervisor>) -> Self {
        let cfg = supervisor.config().clone();
        let ansible = Arc::new(crate::ansible::AnsibleRunner::new(
            cfg.automation_root.clone(),
            cfg.ansible_inventory.clone(),
        ));
        let tests = Arc::new(crate::test_runner::TestRunner::new(
            cfg.project_root.join("src/orchestrator/tests"),
            cfg.log_dir.join("test_history.jsonl"),
        ));
        Self {
            supervisor,
            ansible,
            tests,
            host_status: Arc::new(crate::host_status::HostStatusCache::new()),
        }
    }
}

pub(crate) fn err(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error": message}))).into_response()
}

async fn index() -> Json<Value> {
    Json(json!({"service": "dds-control-panel-supervisor", "version": "0.2.0", "runtime": "rust"}))
}

pub fn router(state: Arc<AppState>) -> Router {
    // Rotas públicas de saúde (mesmo padrão do studio-noded /version):
    // todo o resto exige Bearer token quando configurado.
    let public = Router::new()
        .route("/", get(page_dashboard))
        .route("/services", get(page_services))
        .route("/tests", get(page_tests))
        .route("/chat", get(page_chat))
        .route("/vms", get(page_vms))
        .route("/settings", get(page_settings))
        .route("/api/health", get(index))
        .with_state(state.clone());
    let protected = Router::new()
        .route("/status", get(status_all))
        .route("/services/:name/status", get(service_status))
        .route("/services/:name/start", post(service_start))
        .route("/services/:name/stop", post(service_stop))
        .route("/services/:name/restart", post(service_restart))
        .route("/services/:name/logs", get(service_logs))
        .route("/services/:name/clear-logs", post(service_clear_logs))
        .route("/services/stop-all", post(stop_all))
        .route("/agents/start", post(agents_start))
        .route("/agents/stop-all", post(agents_stop_all))
        .route("/presets", get(presets_list))
        .route("/presets/:preset_id/apply", post(preset_apply))
        .route("/catalog", get(catalog_list))
        .route("/api/dashboard", get(dashboard))
        .route("/api/hardware", get(hardware))
        .route("/api/chat/send", post(chat_send))
        .route("/api/inventory", get(inventory_public))
        .route("/api/ansible/actions", get(ansible_actions))
        .route("/api/ansible/run", post(ansible_run))
        .route("/api/tests", get(tests_list))
        .route("/api/tests/run", post(tests_run))
        .route("/api/tests/run-category", post(tests_run_category))
        .route("/api/tests/run/:run_id", get(tests_run_get))
        .route("/api/tests/history", get(tests_history))
        .route("/api/chat/ready", get(chat_ready))
        .route("/api/chat/stream", get(chat_stream))
        .route("/api/settings", get(settings_get).post(settings_post))
        .route("/api/vms/hosts", get(vms_hosts))
        .route("/api/vms/hosts/status", get(vms_hosts_status))
        .route("/api/vms/hosts/:host_name/refresh", post(vms_host_refresh))
        .route("/api/vms/groups", get(vms_groups))
        .route("/api/vms/playbooks", get(ansible_actions))
        .route("/api/vms/runs", get(vms_runs))
        .route("/api/vms/runs/:run_id", get(vms_run_get))
        .route("/api/vms/runs/:run_id/cancel", post(vms_run_cancel))
        .route("/api/vms/runs/:run_id/stream", get(vms_run_stream))
        .route("/api/vms/playbooks/:action_id/run", post(vms_playbook_run))
        .with_state(state);
    public.merge(protected)
}
