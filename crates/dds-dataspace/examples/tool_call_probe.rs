//! Sonda de ferramentas ao vivo (T-890-06, G-18..22): publica uma
//! `ToolCall.Request` (filesystem.read_file) e observa a MESMA instância
//! evoluir no mesh — claim do gateway, decisão do policy-engine e resultado
//! na instância única (sem tópico de resposta).
//!
//! Pré-requisitos no domínio: `mcp-gateway` (raiz com o arquivo) +
//! `policy-engine` publicando snapshot; requester_id na allowlist da
//! política (ex.: CodeReviewAgent → filesystem.read_file).
//!
//! `cargo run --release -p dds-dataspace --features dds --example tool_call_probe \
//!   -- <domínio> [requester=CodeReviewAgent] [arquivo=prova-t89006.txt]`

use dds_contract::generated::dds_llm_orchestrator::ToolCallRequest;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let domain: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(170);
    let requester = args
        .next()
        .unwrap_or_else(|| String::from("CodeReviewAgent"));
    let file = args
        .next()
        .unwrap_or_else(|| String::from("prova-t89006.txt"));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(probe(domain, &requester, &file))
}

async fn probe(domain: u32, requester: &str, file: &str) -> anyhow::Result<()> {
    use dds_dataspace::api::DataSpaceApi;
    use std::time::Duration;

    let space = dds_dataspace::DataSpace::new(domain, 0)?;
    let call_id = format!(
        "probe-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let call = ToolCallRequest {
        call_id: call_id.clone(),
        request_id: format!("{call_id}-r1"),
        requester_id: requester.to_owned(),
        tool_name: String::from("filesystem.read_file"),
        arguments_json: format!("{{\"path\":\"{file}\"}}"),
        security_level: 0, // PUBLIC (orch-common SecurityLevel)
        status: 0,         // PENDING
        ..ToolCallRequest::default()
    };
    println!("[probe] publicando PENDING call_id={call_id} requester={requester} file={file}");
    // SEM ownership: quem põe o pedido não pode bloquear o gateway
    // (mesma lição do P0-1/T-820 nas Tasks — Ownership=Exclusive).
    space.write_tool_call_without_ownership(call).await?;

    // 45 s: a PRIMEIRA descoberta local↔host via peers unicast custa ~15-20 s;
    // chamadas seguintes são imediatas (participantes já casados).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    let mut last_status = -1;
    while tokio::time::Instant::now() < deadline {
        if let Some(current) = space.read_tool_call_mesh(&call_id)? {
            if current.status != last_status {
                last_status = current.status;
                println!(
                    "[probe] status={} resultado={} erro={:?}",
                    current.status,
                    if current.result_json.is_empty() {
                        "-"
                    } else {
                        current.result_json.as_str()
                    },
                    if current.error_message.is_empty() {
                        None
                    } else {
                        Some(current.error_message.as_str())
                    }
                );
            }
            // terminal no canon: DENIED=2, COMPLETED=4, FAILED=5
            if matches!(current.status, 2 | 4 | 5) {
                match current.status {
                    4 => println!("[probe] OK: ferramenta executada no mesh (COMPLETED)"),
                    2 => println!("[probe] política NEGOU a chamada (DENIED) — governança ao vivo"),
                    _ => println!(
                        "[probe] execução falhou (FAILED): {}",
                        current.error_message
                    ),
                }
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    anyhow::bail!("timeout: instância não chegou a estado terminal em 45s")
}
