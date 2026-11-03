//! Máquinas contra nó e autoridade REAIS em portas efêmeras (sem mocks):
//! REQ/T-840-03 — registro no catálogo compartilhado + sonda `GET /version`
//! com/sem token. O dublê do nó é um servidor TCP bruto no loopback que
//! EXIGE `Authorization: Bearer` até em `/version` (mais estrito que o nó
//! real, onde `/version` fica aberto por T-840-01) para exercitar o caminho
//! 401 da GUI; a autoridade do catálogo é o router de verdade do
//! `studio-node`.

use std::io::{Read, Write};

use orchestrator_studio::catalog_remote::publish;
use orchestrator_studio::machines::{
    machine_catalog_value, parse_machine_value, MachineRecord, MachinesState, ProbeState,
};

/// Token exigido pelo dublê do nó (`&'static` atravessa a thread).
const NODE_TOKEN: &str = "token-orch-62-secreto";

/// Dublê TCP bruto de `GET /version`: responde a versão de protocolo só com
/// o `Bearer` correto; 401 caso contrário (T-840-03c).
fn spawn_version_double(required_token: &'static str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(stream) => stream,
                Err(_) => continue,
            };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 512];
            loop {
                let read = stream.read(&mut chunk).unwrap_or(0);
                if read == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..read]);
                if buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&buf);
            let provided = text.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if !name.trim().eq_ignore_ascii_case("authorization") {
                    return None;
                }
                value.trim().strip_prefix("Bearer ").map(String::from)
            });
            let (status, body) = if provided.as_deref() == Some(required_token) {
                ("200 OK", r#"{"major":1,"minor":0}"#)
            } else {
                ("401 Unauthorized", r#"{"code":"unauthorized"}"#)
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}")
}

/// Autoridade de catálogo REAL (router do `studio-node`, sem token).
async fn live_authority() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    let state = studio_node::server::NodeState::new(Vec::new());
    tokio::spawn(async move {
        axum::serve(listener, studio_node::server::router(state))
            .await
            .expect("autoridade de teste serve");
    });
    format!("http://{addr}")
}

/// Drena o worker do painel como a view faria por frame (REQ/T-820-19),
/// com teto de tempo para falhar rápido se o worker travar.
async fn drain(machines: &mut MachinesState) {
    assert!(
        machines.busy,
        "operação deve sinalizar trabalho em background"
    );
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while machines.busy {
        assert!(
            tokio::time::Instant::now() < deadline,
            "worker de máquinas não respondeu a tempo"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        machines.poll();
    }
}

fn form(machines: &mut MachinesState, machine_id: &str, node_url: &str) {
    machines.form_machine_id = String::from(machine_id);
    machines.form_host = String::from("192.168.1.61");
    machines.form_user = String::from("tese");
    machines.form_node_url = String::from(node_url);
    machines.form_services_hint = String::from("studio-noded");
}

fn machine_of(machine_id: &str, node_url: &str) -> MachineRecord {
    MachineRecord {
        machine_id: String::from(machine_id),
        host: String::from("192.168.1.61"),
        user: String::from("tese"),
        node_url: String::from(node_url),
        services_hint: Some(String::from("studio-noded")),
        registered_at_ns: 1_759_500_000_000_000_000,
    }
}

/// T-840-03b/03c: publicada pela API do catálogo, a máquina aparece no
/// snapshot do painel (várias GUIs veem a mesma lista) — e entradas de
/// outros `kind` são filtradas.
#[tokio::test]
async fn machine_published_via_catalog_api_reaches_panel_snapshot() {
    let authority = live_authority().await;
    let machine = machine_of("orch-62", "http://192.168.1.61:4317");
    let value = machine_catalog_value(&machine).expect("serializa");
    let rev = tokio::task::spawn_blocking({
        let authority = authority.clone();
        let value = value.clone();
        move || publish(&authority, "machine:orch-62", None, &value)
    })
    .await
    .expect("sem panic")
    .expect("cria");
    assert_eq!(rev, 0);
    // Objeto de outro kind no mesmo catálogo: nunca vira linha de máquina.
    let _ = tokio::task::spawn_blocking({
        let authority = authority.clone();
        move || publish(&authority, "proj-x", None, r#"{"kind":"projeto"}"#)
    })
    .await
    .expect("sem panic")
    .expect("cria projeto");

    let mut machines = MachinesState::with_url(&authority);
    machines.refresh();
    drain(&mut machines).await;

    let list = machines.machines();
    assert_eq!(list.len(), 1, "só kind==machine: {:?}", machines.snapshot);
    assert_eq!(list[0], machine);
    assert_eq!(machines.cursor, 2);
}

/// T-840-03c: fluxo completo do formulário — publicar (token vai só para a
/// memória local) → probe com o token guardado → online com a versão.
#[tokio::test]
async fn panel_publish_then_probe_with_saved_token_is_online() {
    let authority = live_authority().await;
    let node_url = spawn_version_double(NODE_TOKEN);
    let mut machines = MachinesState::with_url(&authority);
    form(&mut machines, "orch-62", &node_url);
    machines.form_token = String::from(NODE_TOKEN);

    machines.publish_machine().expect("publica");
    assert!(
        machines.tokens.contains_key(&node_url),
        "token do formulário viva só na memória local"
    );
    drain(&mut machines).await;
    assert!(
        machines.notice.contains("publicado em r0"),
        "{}",
        machines.notice
    );
    let machine = machines.machines().remove(0);
    assert_eq!(machine.catalog_id(), "machine:orch-62");
    // O valor publicado NUNCA contém o segredo (RNF-04).
    let stored = machines
        .snapshot
        .as_ref()
        .expect("snapshot")
        .items
        .iter()
        .find(|item| item.id.0 == machine.catalog_id())
        .expect("item da máquina")
        .value
        .clone();
    assert!(!stored.contains(NODE_TOKEN), "segredo vazou ao catálogo");
    assert!(parse_machine_value(&stored).is_some());

    machines.probe(&machine);
    drain(&mut machines).await;
    let status = machines.probes.get(&node_url).expect("probe");
    assert_eq!(status.state, ProbeState::Online);
    assert!(status.detail.contains("protocolo 1.0"), "{}", status.detail);
    assert!(machines.missing_token_urls().is_empty());
}
