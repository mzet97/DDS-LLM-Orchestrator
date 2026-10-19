# Evidências T-900-M1-04 — interop real Rust↔Python (V02/V03)

Data: 2026-10-17 (UTC). **Gate M1 completo: 4/4 cenários PASS.**

## Componentes

| Arquivo | Papel |
|---|---|
| `crates/spike-interop/src/bin/workflow_interop.rs` (`workflow-interop`) | Publicador/assinante Rust dos 5 tópicos via `DdsWorkflowSpace` |
| `workers/laya-worker/harness/py_workflow_pub.py` | Publicador Python (bindings gerados; QoS idêntica aos perfis Rust; `--wrong-type`, `--hold-s`) |
| `workers/laya-worker/harness/py_workflow_sub.py` | Assinante Python (poll `take()` 50 ms) |
| `workers/laya-worker/harness/run_interop.py` | Driver dos 4 cenários |
| `workers/laya-worker/harness/cyclonedds_loopback.xml` | Config loopback/sem multicast/peers locais (disciplina do M1 REPORT — ver achado abaixo) |

## Resultados ([interop-run.log](interop-run.log), exit 0)

| Cenário | Resultado |
|---|---|
| V02-a Rust sub ← Python pub | **PASS** — 5/5 tópicos com conteúdo (request_id/decision_id/agent_id/plan_revision/state_revision) |
| V02-b Python sub ← Rust pub | **PASS** — 5/5 |
| V02-c mismatch deliberado (RoutingDecision publicado no tópico Workflow.Intent) | **PASS** — nada entregue (typename/TypeIds não fazem matching) |
| V03-a late joiner (sub inicia DEPOIS das publicações; writer vivo por hold) | **PASS** — 5/5 recebidos via TransientLocal |

Interoperabilidade Rust(gerado cyclonedds-build + blobs XTypes do idlc C) ↔
Python(idlpy 11.0.1) **provada em processos reais** nos dois sentidos,
incluindo sequences aninhadas (Plan→PlanNode→InputRef) e campos enum-como-long.

## Achado: descoberta exigia loopback

Com a config global do host (`CYCLONEDDS_URI` do usuário: interface enp4s0 +
peers do cluster + multicast off), processos PYTHON não se descobriam
(nem py↔py; Rust↔Rust funcionava). A disciplina do M1 REPORT (loopback, sem
multicast, peers locais) resolveu: `cyclonedds_loopback.xml` injetada pelo
driver em TODOS os filhos. Nada do ambiente do usuário foi alterado.

## Regressão dos 19 tópicos antigos

- `cargo test -p dds-dataspace --features dds --locked --offline`: **56/0**
  (inclui `t808_dds`: 19 tópicos observados em DDS real).
- `cargo test -p dds-contract --features dds --locked --offline`: **48/0**
  (wire dos 19 + novos 5).
- `topics::ALL` permanece com 19 entradas (teste `workflow_topics_isolated_...`).

## R-INTEROP — resolução parcial com evidência

A divergência "cyclonedds-build descarta bound de string" NÃO impediu o
matching: o build.rs anuncia os TypeIds do idlc C (bounds preservados) via
`dds_type_metadata`, e o idlpy gera os mesmos bounds — entrega bidirecional
confirmada. Resíduo: o LADO Rust não valida bounds de string localmente
(validação de aplicação fica para M2/M3 no intake/engine).

## Limites (não verificado aqui)

- V03-b (writer morto antes do late joiner): contrato NÃO promete retenção —
  documentado, não testado positivamente.
- QoS mismatch deliberado (ex.: durability incompatível) — cenário adicional
  para M3/M4 quando houver superfície de configuração por requisição.
