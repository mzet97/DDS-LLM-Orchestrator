# Plan 400 — Baselines + consolidação (como)

## Onde
Estender a crate `qos-nfcm` com um módulo `baselines` (mesma trait de decisor):
```
crates/qos-nfcm/src/baselines/
├── mod.rs        # trait QosDecider { fn decide(&self, &[f64;8]) -> usize }
├── static_.rs    # REQ-501: perfil fixo
├── zadeh.rs      # REQ-502: score linear ponderado (porte de qos_selector.py)
├── fcm.rs        # REQ-503: motor FCM (sigmoide, atrator) + DHL (porte de fcm_qos_manager)
```
O `Nfcm` também implementa `QosDecider`. O orchestrator escolhe por `--qos-manager`.

## Paridade (Python → Rust)
| Python | Rust |
|---|---|
| `fuzzy_qos_manager/qos_selector.py` | `baselines::zadeh` |
| `fcm_qos_manager/fcm.py` + `dhl.py` | `baselines::fcm` |
| perfil estático | `baselines::static_` |
| `qos-nfcm` (já pronto) | `Nfcm` |

## Harness (REQ-505)
`crates/qos-nfcm/examples/five_arms.rs`: roda os 5 decisores sobre os cenários canônicos +
um trace sintético; imprime a decisão de cada um por cenário (tabela igual à do artigo) e
métricas medíveis localmente (trocas de perfil, convergência do FCM/NFCM). Os números de
desempenho de QoS (latência/erros) vêm do cluster (não inventar).

## Teste
- Paridade por baseline: reproduzir um resultado conhecido do Python (ex.: Zadeh e FCM
  discriminam os cenários; FCM diverge do linear no caso-limite "lote barato").
- E2E (REQ-506) permanece verde após arquivar o Python.

## Consolidação
Arquivar os pacotes Python equivalentes (mover para `archive/`, não apagar) com nota. Manter
a suíte de benchmark/qualificação (mede ambos os mundos).
