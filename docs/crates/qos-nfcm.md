# `qos-nfcm` — decisores QoS (NFCM/fuzzy/bandits)

> Crate: `src/rust/crates/qos-nfcm/` · **[LEGADO — disciplina]** · Lib pura, sem
> bin, sem features, sem DDS · Porte de `src/orchestrator/neuro_fuzzy/` (Python).

## Propósito

Neuro-Fuzzy Cognitive Map + família de decisores para seleção adaptativa e
interpretável de perfil QoS. **Fora do hot path DDS-first**; o orquestrador só
consome com roteamento fuzzy ligado (default OFF). Reproduz os números do artigo
(Seção 8) e discrimina os 4 cenários canônicos.

## Decisores (trait `QosDecider`: `decide` + `name`)

| `name()` | Decisor |
|---|---|
| `nfcm` | `Nfcm`: fuzifica→drives→dinâmica até 100 it→softmax; guarda NaN→Balanced; `NfcmResult` inclui `h_history` p/ figuras |
| `zadeh` | `ZadehDecider`: `FuzzyNumber` por α-cuts, seleção conservadora; fallback total Balanced/`converged=false` |
| `fcm` | `FcmDecider`: mapa cognitivo fuzzy (entradas clampadas, atrator) |
| `fcm-dhl` | `FcmDhlDecider`: DHL online com `Mutex` (a ordem dos cenários influencia) |
| `fixed-rules` | Cadeia if/elif 1:1 com o Python |
| `mamdani` | 27 regras, trimf |
| `ucb1` / `sw-ucb` | Bandits (só histórico de braço; janela default 100 no SW) |
| `static` | Sempre o mesmo perfil |

Entrada: `QoSMetrics` (8 campos, contrato [0,1]). Saída: `QoSDecision` (profile,
confidence, explanation, converged, runner_up). `QoSProfile`: 5 perfis
(0=Critical..4=Balanced; fora-do-intervalo→Balanced). `explain_text()`: trilha
causal ("score", nunca "confiança").

Suporte: `StabilityController` (histerese: margem/persistência/dwell/cooldown/
confiança mínima — defaults são chute documentado); `GaussTerm` (σ=softplus+ε);
`CANONICAL` (4 cenários rotulados) + `synthetic_dataset` (splitmix64+Box-Muller,
Fisher-Yates); `NfcmTrainer` (gradiente numérico **paralelo** via rayon; lr=0,2,
60 épocas); `utility()` multiobjetivo (7 termos) + `best_profile()`.

Nota: baselines fiéis NÃO reproduzem os rótulos NFCM em ocioso/degradado — é a
lacuna que o artigo quantifica.

## Examples e testes

```bash
cargo run -p qos-nfcm --example five_arms --release   # harness 5 braços
cargo run -p qos-nfcm --example demo_decisoes         # 4 cenários + explicação
cargo run -p qos-nfcm --example degradado_convergencia > convergencia.csv
cargo test -p qos-nfcm
```

Examples: `five_arms`, `demo_decisoes`, `degradado_convergencia`, `fcm_check`,
`dhl_probe`, `dhl_probe2`. Testes: unitários + `fcm_parity` (7 iterações
fixed_point, dumps reais), `zadeh_parity` (match 1e-9), robustness.

## Limites

- Legado de disciplina/artigo; fora do hot path.
- Treino custoso: diferenças finitas, O(params) inferências/época (sem autodiff);
  `train_membership=false` por default.
- `Outcome` ainda usa `ttft`/`itl` (vs canônico TTFC/ICL do Gate A).
- `FixedRulesDecider` não clampeia entradas (`decider.rs:20` requer [0,1]).
