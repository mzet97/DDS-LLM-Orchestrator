//! Parte de `dds_observe` (extraída para manter os arquivos revisáveis).

use super::*;

fn sorted<K: Ord, V>(rows: HashMap<K, V>) -> Vec<V> {
    let mut rows: Vec<(K, V)> = rows.into_iter().collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, value)| value).collect()
}

/// Observa o domínio uma janela: um `DataSpace`, sete drenos concorrentes.
///
/// `DataSpace::new` exige contexto Tokio (WaitSet compartilhado) — por isso
/// nasce dentro do `block_on`, nunca fora dele.
pub fn observe(domain: u32, window: Duration) -> Result<DdsSnapshot, ObserveError> {
    let rt = runtime(domain)?;
    rt.block_on(async {
        let space = dataspace(domain)?;
        let (tasks, task_outputs, agents, tools, metrics, discoveries, studio_nodes) = tokio::join!(
            drain(space.stream_tasks(), window, |task: &Task| task
                .task_id
                .clone()),
            drain(
                space.stream_task_outputs(),
                window,
                |output: &TaskOutput| (output.task_id.clone(), output.seq_num)
            ),
            drain(space.stream_agent_states(), window, |state: &AgentState| {
                state.agent_id.clone()
            }),
            drain(
                space.stream_tool_calls(),
                window,
                |call: &ToolCallRequest| call.call_id.clone()
            ),
            drain(
                space.stream_system_metrics(),
                window,
                |metric: &SystemMetric| format!("{}:{}", metric.component_id, metric.metric_name)
            ),
            drain(
                space.stream_discovery_events(),
                window,
                |event: &DiscoveryEvent| event.event_id.clone()
            ),
            // T-890: presença das instalações do Studio (19º tópico).
            drain(
                space.stream_studio_node_presences(),
                window,
                |presence: &StudioNodePresence| presence.node_id.clone()
            ),
        );
        Ok(DdsSnapshot {
            tasks: sorted(tasks).iter().map(task_row).collect(),
            task_outputs: sorted(task_outputs).iter().map(task_output_row).collect(),
            agents: sorted(agents).iter().map(agent_row).collect(),
            tools: sorted(tools).iter().map(tool_row).collect(),
            metrics: sorted(metrics).iter().map(metric_row).collect(),
            discoveries: sorted(discoveries).iter().map(discovery_row).collect(),
            studio_nodes: sorted(studio_nodes).iter().map(studio_node_row).collect(),
        })
    })
}

/// Mensagem do worker de observação.
pub(crate) struct ObserveMsg(Result<DdsSnapshot, String>);

/// Estado do painel de topologia DDS.
#[derive(Debug)]
pub struct DdsState {
    pub domain: u32,
    pub window_secs: u64,
    pub snapshot: DdsSnapshot,
    pub error: String,
    /// `true` enquanto a coleta roda em background (`poll` drena).
    pub busy: bool,
    /// Aba ativa do painel de coleções (0=tarefas, 1=agentes, 2=servers,
    /// 3=tools, 4=métricas, 5=descoberta, 6=instalações) — estado de
    /// apresentação da view.
    pub tab: u8,
    /// Filtro por substring aplicado às tabelas de coleções.
    pub filter: String,
    /// Filtro de status da tela 3.13 (None = todos os status).
    pub tools_filter: Option<i32>,
    /// Call ID selecionada no inspetor da tela 3.13.
    pub tools_selected: Option<String>,
    /// Tarefa selecionada na aba TAREFAS DDS (foca o log de TaskOutput).
    pub task_selected: Option<String>,
    /// Nó/agente selecionado no mesh (card "ASSINANTE SELECIONADO").
    pub mesh_selected: Option<String>,
    pub(crate) receiver: Option<mpsc::Receiver<ObserveMsg>>,
}

impl DdsState {
    /// Padrão honesto: domínio vivo local, janela de 3s para heartbeats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            // Domínio default = domínio da descoberta (env STUDIO_DDS_DOMAIN,
            // default 170 = laboratório) — abrir Topologia/Ferramentas já
            // observa o domínio certo (T-890-03).
            domain: std::env::var("STUDIO_DDS_DOMAIN")
                .ok()
                .and_then(|d| d.trim().parse().ok())
                .unwrap_or(170),
            window_secs: 5,
            snapshot: DdsSnapshot::default(),
            error: String::new(),
            busy: false,
            tab: 0,
            filter: String::new(),
            tools_filter: None,
            tools_selected: None,
            task_selected: None,
            mesh_selected: None,
            receiver: None,
        }
    }

    /// Observa o domínio em THREAD de trabalho (`observe` faz `block_on` de
    /// 1–30 s — nunca na thread de UI, REQ/T-820-19). Erro preserva a foto
    /// anterior e registra o motivo. Clique durante `busy` é ignorado.
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        let domain = self.domain;
        let window = Duration::from_secs(self.window_secs.max(1));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ObserveMsg(
                observe(domain, window).map_err(|err| err.to_string()),
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.error = "observando domínio…".into();
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(ObserveMsg(result)) = rx.try_recv() {
                match result {
                    Ok(snapshot) => {
                        self.snapshot = snapshot;
                        self.error.clear();
                    }
                    Err(err) => {
                        self.error = err;
                    }
                }
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
    }

    /// Tool calls da janela em JSON no temporário (botão Exportar JSON da
    /// 3.13; padrão `SharedCatalog::export_snapshot`).
    pub fn export_tools(&self) -> std::io::Result<std::path::PathBuf> {
        let payload = serde_json::json!({
            "domain": self.domain,
            "window_secs": self.window_secs,
            "tools": self.snapshot.tools,
        });
        let text = serde_json::to_string_pretty(&payload).map_err(std::io::Error::other)?;
        let path = std::env::temp_dir().join("studio-tools.json");
        std::fs::write(&path, text)?;
        Ok(path)
    }
}

impl Default for DdsState {
    fn default() -> Self {
        Self::new()
    }
}
