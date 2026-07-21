# Report 400 — Baselines + consolidação

**Data:** 2026-07-18 · **Status:** ✅ Concluída (6/6 tasks; migração do NÚCLEO consolidada)

---

## O que foi entregue

### T-501 · trait QosDecider
`qos_nfcm::decider::{QosDecider, QoSMetrics, QoSDecision, StaticDecider}` (outra sessão)
+ **`impl QosDecider for Nfcm`** (elo que faltava) — os 5 braços atrás da mesma trait.

### T-502 · Zadeh — porte FIEL (reescrito do zero)
A versão inicial (outra sessão) era uma aproximação linear com pesos errados em
`QoS_Balanced` e semântica invertida. Reescrito como porte fiel de `fuzzy_qos_manager/`:
`FuzzyNumber` (α-cuts, interpolação, centróide), `ExtensionPrincipleEvaluator`
(vértices 2^n, clamp [0,1], aninhamento) e `ZadehSelector` (pesos canônicos;
negativo ⇒ `|w|·(1−val)`; seleção conservadora por (lower_0.8, centroid)).
**Paridade exata vs Python** (`tests/zadeh_parity.rs`, valores gerados pelo Python real):
6 cenários, seleções e centroids por perfil **idênticos (1e-9)**.

### T-503 · FCM + DHL — porte FIEL (reescrito do zero)
A versão inicial iterava sem clamp e degenerava para um atrator único. Reescrito como
porte fiel de `fcm_qos_manager/`: entradas clampadas, detecção de atrator
(fixed_point/limit_cycle/max_iter), arestas ENTRE conceitos, e DHL de Kosko
(`Δw = c_t·(ΔC_i·ΔC_j − w)`, `c_t = c0·decay^t`) com aprendizado online sobre o
**estado completo** (fix: a versão anterior decaía os pesos de decisão para zero —
só via métricas; agora aprende das ativações de decisão também).
**Paridade vs Python** (`tests/fcm_parity.rs`): 6 cenários, vencedor + ativações (1e-4),
7 iterações fixed_point. **Divergência real vs linear em 'lote barato'** (FCM→Balanced,
linear→LowCost); DHL converge para a correlação observada (0,105, validado em probe).

### T-504 · `--qos-manager` com os 5 modos
Orchestrator aceita `--qos-manager {static,zadeh,fcm,fcm-dhl,nfcm}` (default nfcm);
control loop usa `Arc<dyn QosDecider>`. Teste `tests/qos_manager.rs`: os 5 modos rodam
e decidem corretamente no cenário degradado (4 Failover + static Balanced).

### T-505 · Harness de 5 braços
`qos-nfcm/examples/five_arms.rs`: tabela 6 cenários × 5 braços (perfil, confiança,
ns/decide local) + bloco de divergências. Dado honesto medido: zadeh ~190 µs/decide
(enumeração 2^n), fcm ~5 µs, fcm-dhl ~17 µs, nfcm ~2 µs.

### T-506 · Arquivamento + E2E + REPORT
`fuzzy_qos_manager/`, `fcm_qos_manager/`, `neuro_fuzzy/` (+ 10 testes Python
dependentes) movidos para `archive/python_qos_baselines/` com README de aposentadoria
(a flag `--fuzzy-qos` do Python degrada graciosamente). **E2E Rust-only revalidado
após o arquivamento** (HTTP → orq → agente → llama-server → DONE, latency 465 ms).

## Estado final da migração (núcleo)

| Componente Python | Contraparte Rust | Prova |
|---|---|---|
| `dds_backend/` | `dds-contract` + `dds-dataspace` | 14 tipos, TypeIds idênticos; propagação p99 0,077 ms |
| `agent/` | `agent` | E2E 10/10; A/B 0 execução dupla; DdsEngine real |
| `orchestrator/` | `orchestrator` | API, reaper, control loop NFCM, state machine |
| `client/` | `client` | 50/50 concorrentes sem deadlock |
| `llm_gateway/` | `llm-gateway` | pool paralelo, roteamento, cache, 429 |
| `neuro_fuzzy/` | `qos-nfcm` (Nfcm) | números do artigo reproduzidos |
| `fuzzy_qos_manager/` | `qos-nfcm/zadeh.rs` | **paridade exata** (6 cenários) |
| `fcm_qos_manager/` | `qos-nfcm/fcm.rs` | **paridade** (6 cenários) + DHL Kosko |

Benchmark do gate: **Rust 58–156× mais rápido** que Python (RTT p99 0,355 ms vs 55,46 ms).

## Fora do núcleo (próximos passos — WF-8/9)

- Subsistemas da dissertação: policy-engine, mcp-gateway, context-store, observability,
  benchmarks (o contrato DDS deles já está pronto — 14 tipos).
- Números finais para a tese (tabelas Rust vs Python no cluster).
