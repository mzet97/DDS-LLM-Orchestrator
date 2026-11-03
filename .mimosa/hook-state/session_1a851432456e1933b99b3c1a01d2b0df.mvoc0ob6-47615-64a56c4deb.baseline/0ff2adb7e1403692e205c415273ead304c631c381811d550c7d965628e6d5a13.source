//! 3.2 Nó studio-node: verificação headless da tela refeita — cabeçalho
//! SEC:3.2, card de conexão, 3 cards, log de operações + inspetor e card de
//! falhas. O caminho conectado usa um stub HTTP real em localhost (round-trip
//! verdadeiro `refresh → worker → poll → render`, sem dados inventados).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::discovery::{DiscoveredNode, DiscoveryState};
use orchestrator_studio::protected::ProtectedGuard;
use orchestrator_studio::state::AppState;
use orchestrator_studio::views;
use studio_node::operations::OpRecord;
use studio_node::protocol::{AdminOp, OperationId, ProtocolVersion};

/// Sobe um stub HTTP em `127.0.0.1:0` servindo `/version` + `/operations`
/// com tipos reais serializados. Retorna a URL base; a thread do stub morre
/// com o processo do teste.
fn stub_node() -> String {
    let version_json =
        serde_json::to_string(&ProtocolVersion { major: 1, minor: 0 }).expect("version json");
    let ops_json = serde_json::to_string(&vec![
        OpRecord {
            id: OperationId(String::from("op-61-boot-001")),
            op: AdminOp::Bootstrap {
                node_name: String::from("lab-61"),
            },
        },
        OpRecord {
            id: OperationId(String::from("op-61-svc-002")),
            op: AdminOp::SetService {
                service: String::from("llama-server"),
                running: true,
            },
        },
    ])
    .expect("ops json");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                continue;
            }
            // Consome o resto dos cabeçalhos até a linha em branco (GET não
            // tem corpo — responder em seguida; nunca `read_to_end`, que
            // travaria esperando o EOF que só vem após a resposta).
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) if line.trim().is_empty() => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            let (status, body) = match path.as_str() {
                "/version" => ("200 OK", version_json.clone()),
                "/operations" => ("200 OK", ops_json.clone()),
                _ => ("404 Not Found", String::from("{}")),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let mut stream = stream;
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    url
}

/// Sem probe: cabeçalho, card de conexão, estados vazios do log/inspetor e
/// card de falhas com `ESTADO ATUAL: SEM LEITURA`.
#[test]
fn node_renders_header_connection_empty_log_inspector_and_failures() {
    let mut state = AppState::new();
    let mut node_url = String::from("http://127.0.0.1:4317");
    let mut node_token = String::new();
    let discovery = DiscoveryState::new_disabled(170);
    let guard = ProtectedGuard::new();

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (state, node_url, node_token, discovery, guard)| {
            views::node::show(ui, state, node_url, node_token, discovery, guard)
        },
        (
            &mut state,
            &mut node_url,
            &mut node_token,
            &discovery,
            &guard,
        ),
    );
    harness.set_size(egui::vec2(1400.0, 2000.0));
    harness.run_steps(5);

    // Cabeçalho (o badge SEC:3.2 é pintado à mão — fora da árvore accesskit).
    harness.get_by_label_contains("3.2 Nó studio-node");
    harness.get_by_label_contains("Daemon Local por Máquina (Porta 4317)");
    // Card de conexão + linha de status honesta.
    harness.get_by_label_contains("INTERFACE DE CONEXÃO RPC & AUTENTICAÇÃO IN-MEMORY");
    harness.get_by_label_contains("○ Sem leitura");
    harness.get_by_label_contains("Protocolo: v— (OK)");
    harness.get_by_label_contains("MODO: LEITURA (SEM TOKEN)");
    // 3 cards.
    harness.get_by_label_contains("VERSÃO DO PROTOCOLO");
    harness.get_by_label_contains("TOTAL DE OPERAÇÕES");
    harness.get_by_label_contains("MODO DO NÓ & CONCESSÃO");
    harness.get_by_label_contains("URL manual (sem lease DDS)");
    // Log vazio + inspetor vazio.
    harness.get_by_label_contains("Log de Operações Aplicadas");
    harness.get_by_label_contains("Nenhuma operação registrada no nó.");
    harness.get_by_label_contains("Inspetor de Operação RPC");
    harness.get_by_label_contains("Selecione uma operação na tabela ao lado.");
    // Falhas.
    harness.get_by_label_contains("Tratamento de Falhas & Diagnóstico de Transporte RPC");
    harness.get_by_label_contains("ESTADO ATUAL: SEM LEITURA");
    harness.get_by_label_contains("Unauthorized / Token Inválido");
    harness.get_by_label_contains("Falha de Socket TCP / Daemon Offline");
}

/// Conectado ao stub: status vivo, tabela com 2 operações reais e inspetor
/// com encoding + tamanho do payload.
#[test]
fn node_connected_renders_live_log_and_inspector() {
    let url = stub_node();
    let mut state = AppState::new();
    state.refresh_from_node_with_token(&url, None);
    for _ in 0..500 {
        state.poll();
        if !state.busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        state.node().is_some(),
        "stub deve responder /version + /operations: {}",
        state.status()
    );

    let mut node_url = url.clone();
    let mut node_token = String::new();
    let mut discovery = DiscoveryState::new_disabled(170);
    discovery.observe(vec![DiscoveredNode {
        node_id: String::from("nodo-stub-3.2"),
        url: url.clone(),
        token_required: false,
        last_seen_unix_ns: orchestrator_studio::machines::now_unix_ns(),
        probe: None,
    }]);
    let guard = ProtectedGuard::new();

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (state, node_url, node_token, discovery, guard)| {
            views::node::show(ui, state, node_url, node_token, discovery, guard)
        },
        (
            &mut state,
            &mut node_url,
            &mut node_token,
            &discovery,
            &guard,
        ),
    );
    harness.set_size(egui::vec2(1400.0, 2000.0));
    harness.run_steps(5);

    // Linha de status viva.
    harness.get_by_label_contains("● Conectado");
    harness.get_by_label_contains("Protocolo: v1.0 (OK)");
    harness.get_by_label_contains("Latência Round-trip:");
    harness.get_by_label_contains("Socket: ESTABLISHED");
    harness.get_by_label_contains("COMPATÍVEL §31");
    harness.get_by_label_contains("(id na descoberta: nodo-stub-3.2)");
    harness.get_by_label_contains("Lease DDS 10s");
    // Tabela honesta (4 colunas) com as 2 operações do stub.
    harness.get_by_label_contains("op-61-boot-001");
    harness.get_by_label_contains("op-61-svc-002");
    harness.get_by_label_contains("NODE_BOOTSTRAP");
    harness.get_by_label_contains("SERVICE_ACTION");
    harness.get_by_label_contains("unit: llama-server");
    harness.get_by_label_contains("bootstrap lab-61");
    harness.get_by_label_contains("serviço llama-server on");
    harness.get_by_label_contains("EXIBINDO 2 DE 2 REGISTROS");
    // Inspetor com a operação selecionada.
    harness.get_by_label_contains("TRANSPORT_ENCODING");
    harness.get_by_label_contains("application/json (UTF-8)");
    harness.get_by_label_contains("PAYLOAD_SIZE");
    harness.get_by_label_contains("Bytes");
    harness.get_by_label("COPIAR");
    // Falhas reflete o estado vivo.
    harness.get_by_label_contains("ESTADO ATUAL: CONECTADO");
}
