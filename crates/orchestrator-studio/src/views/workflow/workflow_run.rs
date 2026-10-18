//! Parte de `views::workflow` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn run_chain(
    workflow_id: u64,
    config: WorkflowConfig,
    tx: std::sync::mpsc::Sender<WorkflowEvent>,
) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            let _ = tx.send(WorkflowEvent::Done {
                total_ms: 0,
                error: Some(format!("runtime tokio: {err}")),
            });
            return;
        }
    };
    let outcome = rt.block_on(chain(workflow_id, config, &tx));
    let _ = tx.send(outcome);
}

/// Réplica do `run_seq` do `wf-run` com os MESMOS prompts congelados.
#[cfg(feature = "dds")]
async fn chain(
    workflow_id: u64,
    config: WorkflowConfig,
    tx: &std::sync::mpsc::Sender<WorkflowEvent>,
) -> WorkflowEvent {
    use client::dds_impl::DdsClientDds;
    use client::wf_assembly::{self, ANALYSIS, REVIEW};
    use client::{ClientConfig, DdsClient};
    use std::time::Instant;

    let started = Instant::now();
    let config_client = ClientConfig {
        client_id: format!("studio-wf{workflow_id}"),
        dds_domain: config.domain,
        timeout_ms: config.timeout_ms,
    };
    let helper = DdsClient::new(config_client.clone());
    let client = match DdsClientDds::new(config_client) {
        Ok(client) => client,
        Err(err) => {
            return WorkflowEvent::Done {
                total_ms: started.elapsed().as_millis() as u64,
                error: Some(format!("cliente DDS: {err}")),
            };
        }
    };
    let mut prior: Vec<String> = Vec::new();
    let stages = [
        ("A", wf_assembly::SEQ_PROMPTS[0], vec![]),
        ("B", wf_assembly::SEQ_PROMPTS[1], vec![ANALYSIS]),
        ("C", wf_assembly::SEQ_PROMPTS[2], vec![ANALYSIS, REVIEW]),
    ];
    for (name, prompt, labels) in stages {
        let carried: Vec<(&str, String)> = labels
            .iter()
            .zip(prior.iter())
            .map(|(label, content)| (*label, content.clone()))
            .collect();
        let messages = wf_assembly::build_messages(prompt, &config.entry, &carried);
        let task = helper.create_task(&config.model, &messages, 5, false);
        match client.submit(task).await {
            Ok(result) if result.success => {
                let _ = tx.send(WorkflowEvent::Stage(StageOut {
                    stage: String::from(name),
                    task_id: result.task_id.clone(),
                    latency_ms: result.latency_ms,
                    preview: WorkflowState::preview(&result.content, 96),
                }));
                prior.push(result.content);
            }
            Ok(result) => {
                return WorkflowEvent::Done {
                    total_ms: started.elapsed().as_millis() as u64,
                    error: Some(format!(
                        "estágio {name} sem sucesso (task {})",
                        result.task_id
                    )),
                };
            }
            Err(err) => {
                return WorkflowEvent::Done {
                    total_ms: started.elapsed().as_millis() as u64,
                    error: Some(format!("estágio {name}: {err}")),
                };
            }
        }
    }
    WorkflowEvent::Done {
        total_ms: started.elapsed().as_millis() as u64,
        error: None,
    }
}
