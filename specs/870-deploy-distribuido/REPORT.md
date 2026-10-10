# REPORT — specs/870-deploy-distribuido (2026-10-05)

**3/3 tasks ✅** — runtime Rust pós-congelamento (`congelamento-pre-campanha` @ `853c968`) deployado e **prova E2E multi-host executada ao vivo** nos 4 hosts do lab.

## T-870-01 — Estratégia de build: **resolvida empiricamente (sem fallback)**
Binários release locais (glibc 2.43, `CYCLONEDDS_STATIC=1`) **rodam nos hosts Ubuntu 24.04 (glibc 2.39)** — o medo de glibc não se materializou (símbolos exigidos ≤2.39; `CYCLONEDDS_STATIC` remove a libddsc compartilhada). Validado executando o agente release na .64 antes de qualquer deploy. Fallbacks (docker/musl+C/on-host) não foram necessários.

## T-870-02 — Deploy por papel (domínio 170, XML sem xmlns, domínio via CLI)
| Host | Papel | Binário | Execução |
|---|---|---|---|
| .62 | orquestrador | `orchestrator` | `--port 8080 --dds-domain 170 --qos-manager static` |
| .61 | agente (RTX; llama-server fora do ar — engine mock na prova; EXP2 religa) | `agent` | `--engine mock --dds-domain 170 --slots 4` |
| .64 | 2 agentes (EXP-C futuro) | `agent`/`agent-b` | idem, `agent-64a`/`agent-64b` |
| .63 | cliente | `wf-run` + prompts congelados | `--domain 170 --prompts-dir …` |

Registry no .62 enxergou os 3–4 agentes de 2–3 hosts (`/api/v1/agents`).

## T-870-03 — Provas E2E ao vivo
1. **Workflow entre hosts:** `wf-run seq_chain_v1` na **.63** → **completed** (A/B/C ≈ 507 ms cada) com orquestrador .62 e agentes .61/.64 —Tasks, claims, inferência (mock) e TaskOutput cruzando hosts por DDS.
2. **Continuidade pós-morte:** agente da .61 morto → **12/12 workflows subsequentes completaram** via agentes .64 (16+15 claims nos logs deles).
3. **Mid-flight:** agente da .61 congelado (SIGSTOP) durante a rodada → workflow completou 1,6 s (congelado não chegou a clamar — janela de claim 500 ms; o caso "claimou e travou" está coberto pelo E2E automatizado `t820_failover_*` da fase 850).
4. **Ring final:** 3/3 workflows após limpeza.

## Descobertas de deploy (registrar — valem para a campanha 880)
1. **XML com `xmlns="https://cdds.org/config"` é SILENCIOSAMENTE IGNORADO** pelo CycloneDDS vendido (cyclonedds-src 1.0.2): config cai no default (domínio 0 + NIC autodetectada) — sintoma: `domain_id=0` no log do agente com outro XML. Sem xmlns, `Domain id="…"` funciona.
2. **Regra operacional de domínio (empírica, ligada ao binding 1.2.1):** usar **`Domain id="any"` + domínio via CLI** (`--dds-domain`/`--domain`) — combinação validada local e remotamente; fixar id numérico no XML produziu hangs do cliente nos testes (causa a investigar no binding; contorno adotado).
3. **`pkill -f` com padrão presente na própria linha de comando ssh mata a sessão** — usar `pkill -x <nome exato>`.
4. **Registry não expira agente morto** (achado novo, P2): `agent-61` morto segue em `/api/v1/agents` com `health=2`. O reaper/reatribuição funcionam (provas acima); a listagem é cosmética/observabilidade — corrigir em fase futura (expiry no `AgentRegistry`).
5. llama-server da .61 **fora do ar** — EXP2 exigirá religá-lo (build rocm/cuda já existente nos hosts).

## Artefatos
- XML de descoberta (sem xmlns, `id="any"`, `AllowMulticast false`, peers unicast 4 hosts, autodetect NIC): embutido nos comandos do deploy; variantes em `/tmp/e2e-any.xml` (local) e `~/.config/cyclonedds/e2e170.xml` nos hosts.
- Logs: `.62:/tmp/orch-170.log`, `.61:/tmp/agent-170.log`, `.64:/tmp/agent-170{a,b}.log`, `.63:/tmp/wf-*.json`.
- Serviços deixados RODANDO no domínio 170 para a campanha (880): orchestrator .62 + agentes .64a/.64b (agent .61 morto de propósito; agent-63 vivo em mock).
