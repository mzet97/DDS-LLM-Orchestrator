# Cobertura final de gates SDD — fase 800 (Studio local)

**Data:** 2026-09-11 (**complemento 2026-10-04** ao fim). **Evidência:** T-800-01…T-800-23 em `tasks.md` + testes
`--locked` verdes + provas vivas citadas. Legenda: ✅ entregue · ◐ parcial ·
🔒 bloqueado por ambiente (sem 2º host/VM, sem SSH remoto, sem executor de
tools vivo no mesh) · ❌ não implementado (exige backend inexistente).

## Entregues (✅/◐ com prova)

| Gate | Veredito | Prova |
|------|----------|-------|
| G-01 | ✅ | binário `studio` eframe nativo, sem Python/Node |
| G-03 | ◐ | inventário GGUF + `wanted/active/None`; remoto não |
| G-05 | ✅ | bootstrap idempotente, só próprios, singleton systemd |
| G-06 | ✅ | mesmo op-id → `AlreadyApplied` + `reconcile` |
| G-07 | ✅ | `GET /services` só lê; base obsoleta → 409 |
| G-09 | ◐ | SHA-256 listado; sem veredito contra manifesto |
| G-12 | ✅ | pronto HTTP nunca exibido como pronto DDS |
| G-13 | ✅ | duas rotas distintas, destinos corretos |
| G-14 | ✅ | `task_id` único por despacho (vivo: `090c7627…`) |
| G-16 | ✅ | temperatura/limite capturados no corpo (teste de fio) |
| G-17 | ✅ | sem edição de perfil em execução |
| G-23 | ◐ | multi-turn real; sem uso de ferramenta |
| G-24 | ◐ | ação boba → 404; ferramenta não autorizada n/a |
| G-27 | ◐ | nó persiste; reabre sem reexecutar (workflows n/a) |
| G-28 | ✅ | systemd `active`, 1 listener, sem duplicata |
| G-29 | ◐ | ownership 0, nada publicado; writers criados pelo `DataSpace` (desvio registrado) |
| G-30 | ✅ | reconcile por id; erro em vez de saúde falsa |
| G-34 | ✅ | journal em SQLite (T-890-05): restart adota do `operations.db` — provado ao vivo na .62 (apply → restart → reconcile); JSON legado migrado sem destruição (`.imported`) |
| G-35 | ✅ | sem rollback prometido; tombstone explícito |
| G-36 | ✅ | DB guarda só op_id/op/meta (token é env-only, `AdminOp` não carrega segredo); importação hidrata estado sem executar (teste `migration_imports_state_without_executing`) |
| G-39 | ◐ | só aditivo; suite completa do workspace não rodada |
| G-40 | ✅ | `.desktop` + unit + **.deb montado e validado por extração** (`scripts/build-studio-deb.sh`, 2026-10-06); instalação system-wide é decisão do autor |
| G-42 | ◐ | domínio/janela explícitos; seed n/a |
| G-43 | ◐ | built-ins observados; vazio em malha estável |
| G-44 | ◐ | autoridade compartilhada local; sem auth |
| G-45 | ✅ | parado × online via `wanted/active` |
| G-46 | ✅ | conectar só lê |
| G-47 | ✅ | uma GUI vence, outra leva 409 (vivo + fio) |
| G-48 | ◐ | `events_since` existe; GUI não faz auto-diff |
| G-49 | ◐ | eventos contíguos; 410 existe, caminho não exercitado ao vivo |
| G-50 | ✅ | mesmo id retorna; payload distinto recusado |
| G-51 | ◐ | identidade lógica estável; troca de IP não testada |
| G-52 | ✅ | sem fusão heurística em nenhum ponto |
| G-54 | ✅ | DDS externo só lido, sem adoção |
| G-55 | ✅ | sem escrita de incorporação |
| G-56 | ◐ | janela viva; retido aparece como está |
| G-57 | ✅ | tombstone + 410 testados ao vivo |
| G-58 | ✅ | journal persiste em SQLite atômico (transação única, WAL) atrás de `Storage`; sem eleição silenciosa |
| G-59 | ✅ | replay no restart provado ao vivo |
| G-62 | ◐ | serde ignora desconhecidos; sem escrita parcial |
| G-63 | ◐ | painéis locais funcionam sem internet |
| G-67 | ✅ | drift manual visível, sem reversão automática |
| G-68 | ◐ | `take(20)` + releitura manual |
| G-69 | ◐ | estados crus separados; sem semáforo inventado |

## Adendo T-800-21…T-800-26 (2026-09-12, branch `studio/phase-800-node`)

- T-800-21: inventário GGUF assíncrono (lista instantânea + SHA-256 em
  thread com progresso/cancelamento) + navegação lateral §30.
- T-800-22: Visão geral somente leitura (tela inicial, cartões com `stale`
  explícito; vazio sem erro = "nunca lido").
- T-800-23: assistente Subir inferência (plano legível → start idempotente
  → espera active → lista modelos → prova de geração real), prova viva
  em :8082 (Qwen3.5-0.8B, 32 tokens).
- T-800-24/25: `STUDIO_NODE_BIND` (padrão localhost) + `studio-noded`
  sob systemd na .62 e na .61 (RTX 3080); prova viva: protocolo 1.0 nas
  duas, `dds-agent` parado sem pretendido. 3 origens administráveis.
- T-800-26: registro de nós conhecidos na GUI (apelido+URL digitados,
  sem varredura/SSH). G-02/G-04 seguem bloqueados (cadastro SSH fora).
- Esteira (DRAFT, não executar): `.gitea/workflows/ci.yml` +
  `ansible/{deploy,validate}-studio-node.yml` + template systemd.
  Push ao Harbor desabilitado (robot + CA pendentes, F0).

## Bloqueados por ambiente (🔒 — nada a fazer sem 2º host/credenciais)

G-02, G-04, G-08 (colisão entre nós), G-10, G-11 (verificação cruzada),
G-15 (criação), G-18, G-19, G-20, G-21, G-22, G-25, G-26, G-31, G-32,
G-33, G-41, G-53, G-60, G-61, G-64,
G-66, G-70. Causa única: um host só, sem SSH remoto, sem executor de
tools vivo no mesh. Desbloqueio = prover 2º host/VM + executor.

## Notas de desvio honesto

1. G-29: `DataSpace::new` cria writers (pool) mesmo para observar;
   mitigado com ownership 0 e nenhuma publicação. Leitor puro exigiria
   mudar `dds-dataspace`.
2. Descoberta DDS vazia em malha estável é comportamento esperado
   (eventos só em join/leave), não bug provado.
3. GPU: llama ROCm com 7.4GB VRAM + geração real OK; atribuição por
   requisição não medida.

## Complemento 2026-10-04 — T-800-21/22/23 e fase 830

A tabela acima cobria T-800-01…20; as tasks seguintes (todas `[x]`) e a fase
830 (`specs/830-studio-residual-fixes/`) atualizam os vereditos:

- **T-800-21/22/23** (hash GGUF, Visão geral com fonte/stale, assistente
  "Subir inferência" com prova viva): cobrem G-09 (◐→◐: SHA-256 assíncrono
  com progresso/cancelamento; veredito contra manifesto segue ausente) e
  G-69 (◐: fonte/stale explícitos no agregado). Nenhum novo gate 🔒.
- **Fase 830 (T-830-01…07):** G-48 ◐→**◐+**: a GUI agora acompanha eventos da
  autoridade (`Acompanhar eventos`, convergência provada por fio, 410 →
  re-snapshot); auto-diff na reconexão segue manual. G-40 ◐: `.desktop`
  revalidado com `Exec=studio` via PATH. Painel "Catálogo" local residual
  removido (duplicava o Compartilhado em leitura). `studio-noded` persiste
  por padrão (`$HOME/.local/share/studio-node/operations.json`).
- **Incompatibilidade deliberada (T-830-03):** REQ-801 agora é ENFORÇADO —
  journals antigos gravados quando `generation` era ignorada falham no replay
  com `Corrupt` (fail-fast, política "nunca máscara"). Mitigação: apagar o
  journal local (`operations.json` + `*.catalog.jsonl`) e republicar.

## Complemento 2026-10-05 — fase 840 (multi-host, com os 4 hosts do lab)

Com `specs/840-multihost/REPORT.md` (evidências ao vivo em 192.168.1.61/62/64):
- **G-44 ◐→✅ (local/LAN):** `STUDIO_NODE_TOKEN` Bearer com comparação
  constante; bind fora de 127.0.0.1 sem token ≥16 recusa o boot; `/version`
  aberto para sonda. Eleição/replica seguem n/a.
- **G-70 🔒→◐:** catálogo compartilhado cross-host ao vivo (.62 publica na
  autoridade .64; máquina local lê snapshot+eventos). 2ª GUI **gráfica**
  simultânea segue pendente.
- **G-02 🔒→◐:** registro de máquinas no catálogo (`kind:"machine"`, token
  fora por RNF-04) + deploy reproducível (`scripts/deploy-studio-node.sh`,
  musl estático — glibc 2.43→2.39 resolvido). Fingerprint/host-key gerenciado
  segue básico.
- **G-15 parcial:** atuação remota start/stop de `dds-agent` provada ao vivo
  na .61 (applied → active em 1 s; stop → inactive no host); criação de
  unidades NOVAS pela GUI segue pendente.
- Painel "Máquinas" novo na GUI (10→11 seções). Seguem 🔒: G-18..22 (ferramentas),
  G-25/26 (wf-run GUI ✅ 890-07), G-10/11 (GGUF×manifesto ✅ 890-08); G-38/65 implementados com nota de validação (2026-10-06).
- **G-41/42 (mDNS) → NÃO APLICÁVEL (2026-10-05, decisão do autor):** DDS é
  requisito mínimo em toda máquina da implantação; a descoberta de instalações
  do Studio será **nativa do DDS** pelo 19º tópico canônico
  **`Studio.NodePresence`** (padrão AgentRegistry, fase 890 do
  `PLANO_IMPLEMENTACAO_RESTANTE.md`). mDNS não será implementado — ver
  `notes.md` §I.10 (retratação do SDD §34). Fallback manual/catálogo já
  existe (T-840-03).

## T-890 — presença DDS multi-host AO VIVO (2026-10-06)

- **T-890-02 (nó anuncia a si) ✅ com evidência em 3 hosts reais:** `studio-noded`
  musl-estático **com CycloneDDS C estático** (cross-build via musl.cc 11.2.1;
  snapshot ABI `abi-snapshots/x86_64-unknown-linux-musl.rs` — layouts glibc≡musl
  verificados por diff do probe; reinstalação automática pelo deploy script).
  Descoberta **unicast com peers** do lab (XML sem xmlns — o vendored ignora
  xmlns; lição 870), `STUDIO_NODE_DDS_DOMAIN=170`,
  `CYCLONEDDS_URI` com path ABSOLUTO no env (systemd EnvironmentFile não
  expande `$HOME`) e URL `file://` com três barras.
- **T-890-03 (auto-descoberta) ◐→eixo de dados ✅:** leitor headless
  (`examples/list_node_presence.rs`) vê **3 instalações vivas** (.61/.62/.64,
  heartbeat 5 s/lease 10 s) com `node_id` único por host — o `node_id` default
  deriva da autoridade da URL pública (`STUDIO_NODE_PUBLIC_URL`): sem override,
  os 3 hosts colidiam na chave `0.0.0.0:4317` e o KL1 mantinha 1 instância só.
  O mapeamento GUI (`dds_observe::studio_node_row`) é testado; a captura visual
  do painel Máquinas exige display — queda para checagem do autor.
- Fix embutido: `STUDIO_NODE_PUBLIC_URL` (bind 0.0.0.0 atrás de NAT não é
  conectável); `advertised_url`/`default_node_id` puros com testes (44/0 com
  feature dds no studio-node; workspace 449/0 sem dds).

## T-890-07 — wf-run pela GUI (2026-10-06)

- **Entregue (commit `e17416c`):** painel "Workflow (A→B→C)" no Studio — a
  cadeia sequencial canônica executada pelo data space via **lib `client`**
  (caminho sancionado do plano: "via nó ou lib client"), não por subprocesso.
- **Montagem única:** `wf_assembly` promovido do binário `wf-run` para a
  biblioteca; prompts congelados embutidos (`SEQ_PROMPTS`, cópias
  byte-idênticas dos `seq_*_v1.txt` do baseline) — `wf-run` e GUI não podem
  divergir.
- **Padrão de threads da GUI:** worker em thread + mpsc + `poll` por frame
  (T-820-19); runtime tokio `current_thread` criado DENTRO da thread de
  trabalho; worker que morre sem `Done` libera a UI com erro explícito.
- **G-25/26:** a GUI agora também dispara workflows reais (estágios com
  task/latência/prévia e total); o despacho unitário já existia (T-800-11) —
  o gate fica ✅ com o encadeado. Máquina de estado com 6 testes (corridas
  de thread inclusas) compilando com e sem `dds`; 461 passed / 0 workspace.
- **Evidência ao vivo do caminho de execução:** o mesmo caminho
  (`DdsClientDds::submit` → claim → DONE) é o medido nas EXP0–EXP4; smoke
  visual do painel exige display (checagem do autor, como o painel Máquinas).

## T-890-05 — studio-storage: journal SQLite atrás de trait (2026-10-06)

- **Entregue (commit `527ded6`):** `studio-node/src/storage.rs` — trait
  `Storage` (`save`/`load`/`path`) com dois backends: `JsonStorage` (snapshot
  histórico, compat) e `SqliteStorage` (rusqlite **bundled**, transação
  atômica DELETE+INSERT + WAL). `NodeState` carrega `Arc<dyn Storage>`;
  `with_db` despacha por extensão (`.json` → JSON; resto → SQLite). Default do
  nó: `~/.local/share/studio-node/operations.db`.
- **Migração não destrutiva (G-34):** banco virgem + `operations.json` irmão →
  importa (hidrata estado) e renomeia o JSON para `operations.json.imported`;
  qualquer falha interrompe ANTES de tocar o JSON; 2ª carga vem do banco.
- **Import não executa (G-36):** carga só reconstrói estado (`restore`);
  idempotência e conflito de `operation_id` preservados pós-migração.
- **Musl:** sqlite3 bundled cross-compila com o musl-gcc (deploy dos 3 hosts
  validado; binário `static-pie`).
- **Evidência ao vivo (.62):** `POST /apply` → linha em `operations.db`
  (`strings` confirma); `systemctl --user restart` → `/operations/:id` devolve
  o registro adotado do banco.
- **Fora do escopo desta iteração (documentado):** o journal de eventos do
  catálogo (`*.catalog.jsonl`) permanece JSONL por design (stream de eventos
  com `events_since`); migração relacional dele é trabalho futuro se houver
  necessidade real.

## T-890-06 — ferramentas ao vivo no mesh (2026-10-06)

- **Entregue (commit `17a4892`):** `mcp-gateway` rodando na **.64** (17
  ferramentas, sandbox `~/dds-llm-rust/sandbox`) e `policy-engine` na **.62**
  (`policies.json` v2, republish 60 s) — domínio 170, peers unicast, binários
  musl estáticos com CycloneDDS; painel **"Ferramentas"** na GUI com a
  governança de cada chamada (`ToolRow` estendido: requester, nível, status
  canônico `orch-common::ToolCallStatus`, prévia do resultado).
- **Evidência ao vivo (sonda `examples/tool_call_probe`):** ALLOW —
  `CodeReviewAgent`/`filesystem.read_file` → COMPLETED com o conteúdo do
  arquivo do sandbox (.64), decisão `allow` auditable com `policy_version=2`
  publicada pela .62; DENY — requester fora da allowlist → DENIED
  fail-closed. Cadeia completa cruzando 3 máquinas (sonda local → .64 → .62).
- **3 bugs de protocolo reais corrigidos no caminho:**
  1. `ToolCall.Request` com `Ownership=Exclusive` e escrita comum: quem PÕE o
     pedido virava dono e REJEITAVA a evolução do gateway (mesma classe do
     P0-1/T-820 das Tasks) → strength por papel (cliente 10 publica sem
     ownership; gateway 100 reivindica) + `write_tool_call_without_ownership`.
  2. `policy-engine`: o dedupe de conteúdo matava o republish periódico — o
     timestamp nunca se renovava e TODO late-joiner >300 s recebia snapshot
     expirado → `republish()` força publicação no tick (guarda de regressão
     de versão permanece).
  3. `mcp-gateway`: tolerância ZERO de timestamp futuro — com relógios de
     hosts defasados em ~0,2-0,8 s (medido), o snapshot fresco chegava "do
     futuro" e era rejeitado → skew de 5 s no ingest e no evaluate.
- **G-18..22:** os textos dos gates vivem no SDD mestre; o que esta entrega
  prova ao vivo: ferramentas registradas e executáveis no mesh, governança
  por política vigente com decisão auditável, e observação das chamadas na
  GUI. Status atualizado para ✅-com-esta-evidência (validar contra o texto
  do SDD mestre na revisão final).

## T-890-04 — 2ª GUI simultânea (2026-10-06, parte executável)

- **Provado nesta sessão:** DUAS instâncias gráficas do `studio` (eframe/egui,
  `--features dds`) rodando SIMULTANEAMENTE na mesma estação — PIDs distintos,
  60+ s estáveis, zero saída de erro, **sem guard de instância única** (nada
  impede N GUIs). Compilação release com dds: binário 22,2 MB.
- **Semântica de concorrência do catálogo** (o risco real de 2 GUIs) já está
  ✅ pela G-47 (revisão condicional: uma GUI vence, outra recebe 409 — provado
  vivo+fio na fase 840) e G-50 (mesmo id, payload distinto recusado).
- **Resta para o autor (1 min, requer interação visual):** com as duas
  janelas abertas na estação (binário
  `~/.cache/tese-rust-target-fd/release/studio`), apontar ambas para o mesmo
  nó (ex.: .64 com token) e editar o catálogo pelas DUAS — a revisão
  condicional deve deixar uma vencer e a outra receber 409 na UI. Com esse
  passo, G-70 ✅.

## T-890-08 (item G-40) — pacote .deb do Studio (2026-10-06)

- **Entregue:** `scripts/build-studio-deb.sh` — compila o `studio` release
  (feature dds, CycloneDDS estático, target fora da árvore SMB), monta o
  payload (`/usr/bin/studio` + `.desktop` + copyright) e monta o .deb.
- **Formato sem dpkg-dev (Fedora):** o .deb é montado NA MÃO — `ar` com
  `debian-binary` (2.0) + `control.tar.gz` (control+md5sums) +
  `data.tar.xz` (usr/) — exatamente o que o `dpkg-deb --build` produz; se
  o `dpkg-deb` existir, ele é usado.
- **Depende honesta:** extraída do binário (`objdump -T` → maior símbolo
  GLIBC): `libc6 (>= 2.43)`.
- **Validação por EXTRAÇÃO** (não instala no sistema — instalar é decisão
  do autor: `sudo dpkg -i dist/dds-orchestrator-studio_0.1.0_amd64.deb`):
  `ar t` na ordem canônica, payload extraído, binário executável,
  `.desktop` byte-idêntico e `desktop-file-validate` ✓ (sem warnings).
  Artefato: `dist/dds-orchestrator-studio_0.1.0_amd64.deb` (6,8 MB,
  fora do git).

## T-890-08 (item G-09..11) — GGUF × manifesto congelado (2026-10-06)

- **Entregue (commit `9a21dd6`):** cruzamento GGUF×SHA no painel de modelos —
  `ModelsManifest` (formato canônico `{"models": {"arquivo.gguf": "<sha256>"}}`,
  validação 64 hex case-insensitive), `ManifestStatus`
  (Pendente/Ok/**Desviado**/SemRegistro) por artefato, coluna colorida na GUI
  (OK verde, DESVIADO vermelho) e campo "Carregar manifesto" com default no
  lock da campanha.
- **Manifesto canônico versionado:** `benchmarks/orchestration/locks/models-
  manifest.json` com o SHA congelado do `Qwen3.5-0.8B-Q4_K_M.gguf` (do
  `manifest-freeze.md` da fase 860).
- **PROVA AO VIVO:** o arquivo real (`models/Qwen3.5-0.8B-Q4_K_M.gguf`,
  508 MB) hasheia **byte a byte** ao SHA congelado
  `bd258782…1dc517` (4,6 s) — o artefato usado na campanha EXP2 é o
  congelado. Classificação exercida por 5 testes (parse/validação, OK/
  DESVIADO/SemRegistro e integração poll→status nas duas direções).
- Cobertura: **467 passed / 0** no workspace.

## T-890-08 (item G-37) — kittest: testes de UI automatizados (2026-10-06)

- **Entregue (commit `1f29f27`):** `tests/kittest.rs` no Studio com
  **egui_kittest 0.36.2** (par do egui 0.36) em `default-features = false` —
  testes de INTERAÇÃO pela árvore accesskit (clicar/ler rótulos), 100%
  headless, sem GPU e sem diffs de imagem.
- **3 testes de fluxo real:** (1) painel de Modelos ponta a ponta pela UI
  (clicar "Inventariar" → "Carregar manifesto" → status "OK" visível e
  contagem "1 registro(s)"); (2) DETECÇÃO DE DESVIO pela UI (arquivo
  adulterado → "DESVIADO"); (3) orientação honesta do Workflow sem `dds`
  (cfg not-dds).
- **Refatoração habilitante:** `views` promovido do bin para a lib
  (`pub mod views;`) — os painéis passam a ser testáveis por integração;
  caminhos `crate::` nas views.
- **MSRV — exceção dev-only documentada:** o egui_kittest 0.36.2 exige
  rustc ≥1.95; o código de PRODUÇÃO do workspace permanece 1.85 — só
  `cargo test -p orchestrator-studio` (que compila o dev-dep) exige
  toolchain ≥1.95. G-37 ✅.

## T-890-08 (item G-38/65) — modo protegido (2026-10-06)

- **Entregue (commit `a354ad0`):** `protected.rs` — `ProtectedGuard` com
  default **DESARMADO** (abrir a GUI nunca habilita efeitos). Ações com
  efeito real em máquinas remotas (actuação de unidades systemd no painel
  de Serviços) só disparam com o modo ARMADO no toggle 🛡 da barra
  lateral — e, mesmo armado, exigem **confirmação explícita por ação**
  ("CONFIRMAR: …" → ✔ Confirmar / ✘ Cancelar). Desarmado, a recusa é
  tipada, registrada e NADA trafega.
- **Testes:** 3 unit do guard (recusa/pedido/confirmar/cancelar/desarmar)
  + 2 kittest pela UI real — desarmado recusa sem aceitar payload; armado
  exige "✔ Confirmar" e SÓ dispara a actuação depois (provado com erro de
  rede em URL inalcançável: a chamada FOI feita, e só após o clique).
- **G-38/65:** textos dos gates vivem no SDD mestre — implementação segue
  a evidência do repositório ("modo protegido/benchmark" no REPORT 800);
  **marcada para validação contra o SDD mestre na revisão final**.
- Cobertura: **475 passed / 0** no workspace (5 testes novos); studio
  73/0 sem dds, 79/0 com dds.

## T-890-03 (complemento) — descoberta AUTOMÁTICA no boot da GUI (2026-10-06)

- **Pedido do autor:** "assim que abrir o studio ele deveria já identificar os
  nodes e conectar". Entregue (commit `46a7f91`): worker em background escuta
  `Studio.NodePresence` continuamente no domínio (env `STUDIO_DDS_DOMAIN`,
  **default 170** = laboratório) desde o boot; painel Máquinas ganhou a
  seção "Instalações descobertas automaticamente" no topo — node_id, url,
  probe automático de `/version` (● online com protocolo / 401 token /
  offline) e idade do heartbeat; morto = sem heartbeat >20 s (lease 10 s +
  tolerância de relógio entre hosts).
- **Auto-conexão:** o probe usa o token de `~/.config/studio/hosts/<host>.token`
  (o mesmo arquivo do deploy) — nós conhecidos aparecem **● online com
  protocolo** sem nenhum clique.
- 4 testes novos (merge/ordenação, janela de vida, token por host, snapshot)
  + 2 kittest do modo protegido; **479 passed / 0** workspace (studio 77/0
  sem dds).

## T-890-03 (complemento 2) — alvo ÚNICO compartilhado por todos os painéis (2026-10-06)

- **Pedido do autor:** "nem toda aba do studio está usando a descoberta
  automática". Entregue: o nó selecionado na descoberta (auto = primeiro ●
  online; manual = combobox no painel Máquinas) **propaga a cada frame**
  para TODOS os painéis que falam com um studio-node — Nó (node_url),
  Serviços, Catálogo compartilhado e Máquinas. A barra de status mostra
  "🛰 alvo: URL". Trocar o alvo uma vez muda a GUI inteira.
- Painéis de ORQUESTRADOR (Agentes/Despacho apontam o orquestrador HTTP
  :8080/.62, não um studio-node) e os painéis DDS (Topologia/Ferramentas/
  Workflow usam domínio, não URL) ficaram fora por design — alvo de nó só
  faz sentido onde o par é um studio-node.

## T-890-03 (v2) — inventário vivo COMPLETO + painéis que carregam sozinhos (2026-10-06)

- **Pedido do autor:** "não identificou o serviço de inferência… a única
  aba certa é Máquinas". Entregue (commit `74285ac`): o worker de descoberta
  agora drena **3 streams** no mesmo domínio — `Studio.NodePresence`
  (instalações), `AgentRegistry` (agentes; poda >30 s sem heartbeat) e
  `ServerStatus` (**servidores de inferência**) — num snapshot único.
- **AUTO-CARGA real:** trocar o alvo dispara as leituras sozinhas — Nó
  conecta (token do deploy), Serviços lê o plano, Catálogo tira o snapshot.
  Sem clicar em "Conectar"/"Ler plano".
- **Agentes**: seção DDS ao vivo sempre populada (independe do orquestrador
  HTTP). **Inferência**: seção `ServerStatus` (llama-server com `LLAMA_DDS=ON`
  aparece sozinho; nota honesta: o contrato não carrega URL HTTP).
- **Topologia/Ferramentas**: domínio default 170 (`STUDIO_DDS_DOMAIN`) +
  observação automática ao abrir a aba. **Workflow**: default 170. Barra de
  status: "🛰 descoberta: N nó(s) · N agente(s) · N inferência(s)".
- **Evidência ao vivo:** boot do binário → log `studio: alvo automático:
  http://192.168.1.61:4317` (auto-seleção + auto-carga disparadas).
- **Achado operacional:** instâncias do studio RODANDO interferem no teste
  `llm_result_backlog` (timing; 1 amostra duplicada) — fechá-las antes de
  `cargo test --workspace`; serial (`--test-threads=1`) sempre verde.

## T-890-03 (v3) — logs do Studio + validação contra as VMs (2026-10-06)

- **Pedido do autor:** "verifique nas VMs se realmente tem coisa rodando para
  validar o discovery e adicione logs no studio". Entregue (commit `5513486`).
- **Validação contra as VMs (real):** .61 = llama-server (8082) + studio-noded;
  .62 = **orquestrador (domínio 170, :8080 localhost)** + policy-engine +
  studio-noded; .64 = studio-noded + mcp-gateway. **Causa-raiz da inferência
  invisível:** o llama-server da .61 rodava **sem `--enable-dds`** — o bridge
  nunca subia e `ServerStatus` não era publicado (a descoberta estava CORRETA;
  o serviço é que não anunciava). Reiniciado com `--enable-dds --dds-domain
  170` → no ar. Agente `agent-lab-01` iniciado na .64 (engine dds, domínio
  170). Sonda `examples/domain_probe` (nova): **3 nós + 1 agente + 1
  inferência** — ground truth e GUI passam a bater.
- **Logs:** `studio_log` (ring buffer 500, INFO/WARN/ERRO → stderr E painel
  "Logs" na navegação). Instrumentado: worker (início/escuta/erro DataSpace),
  NOVO nó + resultado do probe, agente novo, inferência nova, poda por
  heartbeat >30 s, alvo automático, auto-carga (token sim/não), observação
  automática da Topologia.
- **Log do boot real (`/tmp/studio-v4.log`):** worker → 3 NOVOS nós (probe
  "protocolo 1.0") → alvo automático .61 → auto-carga com token → agente →
  inferência (~5 s, late-joiner TransientLocal).
- **Observação para o autor:** o orquestrador da .62 só escuta em
  `127.0.0.1:8080` — os painéis Agentes/Despacho (HTTP) não o alcançam da
  estação; ouvir 0.0.0.0 (com token) ou usar os painéis DDS. A descoberta de
  ORQUESTRADOR não existe no contrato (sem tópico de presença dele) — os
  painéis DDS (Agentes ao vivo/Topologia/Ferramentas) cobrem a visualização.

## T-890-03 (v4) — Visão geral consciente da descoberta + binário instalado corrigido (2026-10-06)

- **Causa-raiz do relato "não usa descoberta" (2ª rodada):** o
  `~/.local/bin/studio` executado pelo menu/.desktop era um **binário velho**
  (411 MB, dinâmico, da era 800/840 — zero strings de NodePresence). Substituído
  pelo build atual (23 MB, estático, `--features dds`); validado por strings e
  por captura de tela na estação do autor.
- **Visão geral agora mostra o sistema vivo:** tile Nó = "conectando ao alvo
  X…" durante a auto-carga; tile Agentes = "N agentes no domínio via descoberta
  DDS"; **novo tile** "Descoberta (domínio, ao vivo)" com instalações/agentes/
  servidores de inferência. `OverviewInput` estendido.
- Confirmado visualmente pelo autor na estação (captura): status bar com
  descoberta 3 nós · 2 agentes · 1 inferência e alvo .61.

## T-890-03 (v5) — redesenho visual da Visão geral (2026-10-06)

- **Pedido do autor:** "o design está uma merda". Entregue (grid de cards com
  estado visual): `OverviewTile.health` (Ok/Warn/Stale) mapeado por fonte;
  grid 2 colunas com ícone ● (verde)/◐ (âmbar)/◌ (apagado) + filete colorido
  à esquerda do card; rodapé com alvo 🛰 e resumo do domínio.
- Semântica: Agentes com contagem da descoberta DDS = Ok (não mais "nunca
  lidos" quando o domínio os viu); Inferência com ServerStatus no domínio =
  Warn (presença confirmada, sem prova de geração ainda); Serviços = Warn
  "carregando plano…" durante a auto-carga.
- Validado por captura na estação do autor. 480 passed/0.

## T-890-03 (v6) — MIGRAÇÃO STITCH → EGUI COMPLETA (2026-10-06)

- **Fase 0 (`d2e46f7`):** design system em `theme.rs` (tokens do DESIGN.md
  verificados 1:1 contra o tailwind.config dos mockups: superfícies
  #0f141b→#343941, texto #dee2ec/#bac9cc, accent ciano #00e5ff, semântica
  OK #10B981/WARN #F59E0B/ERROR #EF4444/AUTH #A855F7/STALE #6B7280);
  fontes **Inter + JetBrains Mono embutidas** (include_bytes, fallback
  emoji preservado); TextStyles (headline 16/body 13/button 12/small 11/
  mono 11); `kit.rs` (status_dot, badge tint 10%, banners, empty_state,
  grid_header, mono_cell); shell: header 40px (badge de domínio com dot
  pulsante + chip do modo protegido), sidebar 256px em 5 grupos
  (NAV_GROUPS), status bar 52px em 3 micro-rows mono.
- **Fase A (`7419eb0`):** Máquinas hero (header de escuta com lease/poda/
  heartbeat, cards por instalação com probe ●/🔒/◌ e idade FRESCO/VIVO/
  EXPIRANDO, ação de definir alvo); Visão geral hero (3 números grandes);
  Topologia com **mesh desenhado em Painter** (grade de pontos 16px,
  estação GUI no centro, nós em círculo com linhas SPDP) + coleções.
- **Fase B (`b9ef115`):** Serviços com interlock banner + confirmação
  2 passos em banner âmbar (âncora kittest atualizada); Inferência com
  split DDS×HTTP (3.3 do mockup).
- **Fase C (`056bbcb`):** Ferramentas com filtro por status canônico e
  semáforo de security level; Logs com contadores FIFO coloridos.
- **Fase D:** gates verdes — **481 passed / 0** workspace com
  `CYCLONEDDS_STATIC=1`; kittest 3+2+1 suites ✅; build release
  substituindo `~/.local/bin/studio` (o .desktop executará o novo).
- **Nota de validação:** captura de tela veio no screensaver (tela apagada
  do autor); validação final feita via kittest headless (interação accesskit)
  + os logs em tempo real. Autor valida visualmente ao voltar.

## T-890-UX — CORREÇÃO TELA A TELA SOBRE OS 14 MOCKUPS (2026-10-06)

**Gatilho:** autor reprovou a fidelidade ("não está como no protótipo") — 6
views ainda no layout antigo, `kit.rs` quase todo morto, colunas minúsculas,
URL manual em vez do alvo único. Fonte de verdade: os `code.html` do
`stitch_dds_orchestrator_studio_ux_redesign/` (14 telas; as 8 imagens do autor
= 3.11/3.1/3.10/3.8/3.7/3.12/3.3/3.5, confirmado por conteúdo).

**Decisões do autor na sessão:** tela inicial = **Topologia DDS** (Visão Geral
permanece no grupo Sistema). Dados dos mockups que são INVENÇÃO ficam fora
(GUID, RTT, host/tópicos por agente, tok/s de barramento, VRAM/temperatura,
MD5, % LOSS, presets, "Simular Queda", estimativa de despacho). Níveis de
segurança pelo contrato: `SecurityLevel` 0=PUBLIC/1=INTERNAL (só esses
confirmados no IDL — READ_ONLY/SANDBOX/HOST_MUTATION do mockup descartados).

- **Etapa 0 (`6f7de6b`):** kit completo (metric_card/accent_card/target_chip/
  legend/progress_line/section_label adotados nas views); shell: git hash no
  badge de versão (build.rs), botão **Auto-Carga (5s)** funcional (refresh
  periódico + repaint), sidebar numerada 3.1–3.14 com tags mono
  (DOM/MESH/HB/SYSTEMD/REV/…), `state.poll()` no loop principal (o header
  parava em "conectando…" fora da tela Nó — bug real corrigido).
- **3.11 Topologia REFEITA (`6f7de6b`):** kicker com QoS do contrato, janela
  em chips 1–30 s, **6 contadores reais**, mesh Painter (estação local no
  centro, nós no arco com idade de HB, enxame IA à direita, ServerStatus à
  esquerda, linhas rotuladas), **5 abas** (Agentes/Tool Calls/Métricas/
  Descoberta/Instalações) com filtro por substring (tab/filter no DdsState).
- **3.1 (`6f7de6b`):** 6º cartão Catálogo (cursor+registros reais), legenda
  "N Ok · N Warn · N Stale", botões de ação reais (Re-carregar alvo / Ler
  plano / Atualizar orquestrador), tabela de instalações.
- **3.10 (`6f7de6b`):** tabela canonical (Estado/NodeID | URL | Probe&Versão |
  Autenticação Bearer | Heartbeat&Ações), legenda 3 cores (401 roxo AUTH ≠
  offline), gauges de lease, classificador, catálogo persistido com badges
  TOKEN_OK/AUTH_PEND/OFFLINE, **modal Definir Alvo & Token** (remember_token
  em memória + comuta alvo + re-probe), "Forçar probe" de todos os nós.
- **3.8 (`6f7de6b`):** banner de intertravamento na tela (ARM/DISARM espelha
  o guard; kittest clica ARM e arma de verdade), colunas uppercase,
  SINCRONIZADO/DIVERGE(wanted/active)/DESATIVADO, **↺ Reiniciar = stop→start**
  em worker único (ServicesMsg::Restarted), auditoria em memória (32
  entradas, rotas reais POST /services/<u>/<ação>), última sincronização.
- **3.7 (`6f7de6b`):** colunas separadas checksum calculado × manifesto,
  filtros Todos/Divergentes/Pendentes com contagem, 4 cards resumo,
  "Exportar relatório" (JSON no diretório de modelos).
- **3.12 (`6f7de6b`):** 3 stage cards com conectores e estados derivados
  (pendente/executando/concluído/falhou por parse honesto do erro), 4 métricas
  (total com breakdown A+B+C, estado, 3/3 entregues, último erro), auditoria,
  painel de exceção com o erro REAL.
- **3.3 (`6f7de6b`):** servidores DDS como cards (KV slots com cor de
  lotação), "Verificar /v1/models" com resultado, **top_p atravessa o fio**
  (ChatRequest + asserção no teste de fio), stats reais por resposta
  (duração medida + tokens do usage + tok/s derivado; TurnStats), rodapé de
  slots reais. `chat_completion` mantém assinatura (delega em
  `chat_completion_with_stats`).
- **3.5 (`6f7de6b`):** Seção A (FONTE 1: DDS) e B (FONTE 2: HTTP auxiliar)
  com badges; colunas host/concluídas/falhas do `AgentInfo` real; **taxa de
  sucesso** derivada + linha agregada do enxame; banner de falha honesto
  ("o barramento DDS continua operando").
- **Etapa 2 (`f1d7239`):** 3.13 Ferramentas (cards de nível com contagens,
  chips de filtro por status, inspetor da chamada; tools_filter/
  tools_selected no DdsState); 3.14 Logs (KPIs, busca substring, filtros por
  nível, auto-scroll, **Limpar/Exportar .log** — LogsPanel+clear+export no
  studio_log); 3.2 Nó (select de endpoint da descoberta, tríade de métricas,
  ops com tipo + inspetor, card de falha 401×rede); 3.9 Catálogo (**banner
  409 OCC de 1ª classe** com "Atualizar Snapshot & Mesclar", strip
  cursor/registros/kinds, validação JSON da gaveta, filtros por kind, feed de
  mutações); 3.6 Despacho (card estruturado com agente/latência/conteúdo —
  content extraído do fio — e histórico da sessão com taxa/média); 3.4 Subir
  (CLI preview copiável, wizard com duração **medida** por etapa —
  StepResult.duration_ms, "Abrir no Chat (3.3)").
- **Fix real:** `theme::tint` fazia `alpha10*10` em u8 e **estourava** com
  alpha>25 (overflow no mesh em debug) — agora satura em 255.
- **Gates (`f1d7239`):** fmt OK; clippy `-D warnings` OK ×2 features;
  workspace **483 passed / 0 failed** (110 suítes, `CYCLONEDDS_STATIC=1`,
  GUI fechada e studio-noded preservado); kittest_phase_b 3/3 (topologia
  headless, ARM pela tela, 6º cartão). Bin release glibc+dds 24,8 MB em
  `~/.local/bin/studio`; boot validado no lab: **abre na Topologia**,
  observação automática no boot, 3 nós + 2 agentes + inferência descobertos
  em ~1 s, alvo .61 auto-carregado com token.
- **Validação visual:** captura da tela real (Spectacle) confirma sidebar
  numerada, 6 contadores e mesh renderizando; detalhe por tela cabe ao autor.

## T-890-UX2 — ADEQUAÇÃO AO PRD v1.0 + DESIGNS STITCH ATUAIS (2026-10-06)

**Gatilho:** autor reprovou a 1ª rodada ("ainda está anos distante da
interface que passei") e aprovou o **PRD v1.0** (Outubro 2026), que PROMOVE
a requisito features antes descartadas como invenção de mockup (RTT, wizard
5 etapas, níveis N0/N1/N2, decomposição fila×geração, inspetores
request/response e de logs). Fonte de verdade revisada: projeto Stitch
`5206793100045730557` (atualizado 2026-10-07 — MAIS NOVO que os HTMLs
locais; 14 PNGs 2048px espelhados via MCP `get_screen`, analisados por
visão). Fatos de código que viabilizaram tudo GUI-side, sem tocar backend:
`/api/v1/chat/completions/sync` JÁ devolve T1–T6 (`t_agent_queue_ns`/
`t_inference_ns`/transportes/serialização); `DataSpace` expõe
`stream_tasks`/`stream_task_outputs`; `ToolCallRequest` carrega
`arguments_json`+`result_json` íntegros + timestamps de duração.

**Retratação parcial da T-890-UX:** os rótulos de nível 0=PUBLIC/1=INTERNAL
foram substituídos pelo vocabulário do PRD (`N0 READ_ONLY`/`N1 SANDBOX_EXEC`/
`N2 HOST_MUTATION`) — o PRD sobrescreve a cautela de contrato na GUI.
RTT deixou de ser "invenção": é a duração medida do probe HTTP (EMA α=0,3).
Continuam FORA por não existirem: GUID/participante RTPS, socket fd, Git
SHA/LLVM do processo, VRAM/temperatura, fila de 1024 slots, backoff
exponencial, timestamp/retorno por OpRecord (o nó não carimba — dito na
tela 3.2).

- **`cdd8b56` (ETAPA A — coletores):** `ToolRow` += `arguments_json`/
  `result_json` (cap 8 KB) + `duration_ms` (completed−created); NOVOS
  `TaskRow`/`TaskOutputRow` drenados por `stream_tasks`/
  `stream_task_outputs` (7 drenos concorrentes na mesma janela);
  `workload` captura T1–T6 → `LatencyBreakdown{queue,inference,transport,
  serial}`; hash com **taxa MB/s medida + ETA + cancelamento
  intra-arquivo** (`hash_file_with_progress`, flag por bloco); `LogEntry`
  += `source`/`payload`/`origin` (file:line via `#[track_caller]`)/
  `thread`/`offset_ms`/`slot` + **backtrace force-capturado em ERRO**;
  `AppState` mede RTT do probe (+EMA); `InferenceState.pending_since`.
- **3.11:** aba 0 = **TAREFAS DDS** (ciclo PENDING→ASSIGNED→RUNNING→DONE/
  FAILED, prioridade/retry/idade; ID 8-hex), **log de tarefas** no rodapé
  (TaskOutput, prévia 96, filtro pela tarefa selecionada), **chips de
  filtro por tópico canônico com contagem** (6 tópicos → aba), mesh com
  cards **clicáveis** → card **ASSINANTE SELECIONADO** (dados reais +
  tópicos que publica por papel), aba nova **Server Status** (7 abas).
- **3.13:** cards N0/N1/N2 + **TAXA REQ/s** (janela) + **BLOQUEIOS**;
  colunas DURAÇÃO (real) e POLÍTICA DE DECISÃO; inspetor **REQUEST
  (arguments_json) × RESPONSE (result_json)** lado a lado + COPIAR RAW.
- **3.14:** colunas SUBSISTEMA/OFFSET/SLOT # + **inspetor lateral**
  (thread, fonte file:line, mensagem crua, stack trace, contexto JSON).
- **3.6:** card **TOTAL = FILA + GERAÇÃO + TRANSPORTE + SERIAL** (ms reais
  do `/sync`) com barra proporcional; histórico += FILA/GERAÇÃO.
- **3.2:** faixa CONECTADO · PROTOCOLO · **RTT medido+EMA** · OPERAÇÕES ·
  MODO; inspetor de op com JSON + **COPIAR RAW**; nota honesta sobre
  OpRecord sem timestamp.
- **3.4:** wizard nas **5 etapas canônicas do PRD** (① unidade&binário ②
  spawn systemd ③ processo vivo ④ modelo&KV ⑤ prova+ServerStatus ao vivo
  via chip da descoberta); **console de saídas reais** das operações.
- **3.10:** pill CICLO DE VARREDURA com **contagem real** (5 s − heartbeat
  mais fresco) + badge RTPS MULTICAST 239.255.0.1:7400.
- **3.3:** **GERANDO · N.Ns** vivo (repaint 100 ms) + **FILA real** do
  ServerStatus (idle/processing/PRONTO).
- **3.12:** estágios canônicos **A · Triagem & Fatos / B · Síntese LLM /
  C · Validação & Formatação** (cards, auditoria).
- **3.8:** confirmação em 2 passos agora **VERMELHA** (âncoras literais
  preservadas); **GETs manuais de "Ler plano" entram na auditoria**.
- **NFR/DoD:** `kit::num_cell` (right-to-left + mono) aplicado às colunas
  numéricas de TODAS as tabelas; **chip do alvo único em todas as telas**
  (Nó/Catálogo/Despacho/Inferência/Subir + header global).
- **Gates:** fmt + clippy **0/0 nas duas features**; workspace **507
  passed / 2 ignored** (dds; default 85/0); **kittest_ux2 4 novos**
  (tarefas+log TaskOutput com injeção, inspetor req/resp, subsistema/slot
  dos logs, decomposição T1–T6) — `#![cfg(feature="dds")]` como
  `dds_live.rs`. Binário glibc dinâmico + ddsc estático 23,8 MB em
  `~/.local/bin/studio` (musl segue só para hosts remotos — openssl-sys
  não cross-compila sem vendor); boot validado no lab (3 nós + 2 agentes
  + inferência, alvo .61 auto, log no formato novo `studio[INFO][GUI]`);
  captura em `logs/studio_ux2_topologia_2026-10-06.png` — análise visual
  sem defeitos (sem sobreposição/corte).

## T-890-UX3 — REFORMA VISUAL (2026-10-07, commit `0a8a72a`)

**Gatilho:** autor reprovou a UX2 ("ainda está a mesma merda") MESMO com o
binário novo confirmado em execução (cmp + /proc/exe + hash no header) — o
diagnóstico por comparativo lado a lado (captura real × PNG 2048px do
Stitch) mostrou que UX1/UX2 entregaram a INFORMAÇÃO, não o DESIGN: números
de métrica ~13px vs 36–40px do mockup, cards sem borda, badges retangulares
sem preenchimento, cabeçalho de tabela sem faixa, sidebar sem estado
selecionado, densidade 2× maior que o mockup.

**Execução (100% estilo — nenhum texto/dado/âncora mudou):**
`theme.rs` (escala tipográfica 20/14/13/12/12.5, espaçamentos 16/24/32,
raios 4/6/10+PILL, hover ciano, janela 1500×950) · `kit.rs` (metric_card
HERO 32px+borda, badge PILL tinta18+borda, grid_header em faixas, novo
`kit::table` zebra+28px, `primary_button`/`pill_chip`) · `panel_header`
faixa hero com filete ciano · `main.rs` sidebar selecionado = barra ciano +
tinta, topbar em pills · mesh com cards de assinante 148×38 + dot de status
· 20 grids → `kit::table`.

**Autovalidação visual (NOVA — imposta nesta fase):** captura real × design
3.11 composta e analisada por visão ANTES de entregar. Ciclo 1: 75%
(sidebar discreta, faixa pequena, mesh pobre) → ciclo 2 corrigiu os 3
resíduos e a verificação final confirmou todos os itens (dashboard lê como
desenhado). Comparativo arquivado em `logs/studio_ux3_vs_design_2026-10-07.png`.

**Gates:** 101/0 (dds) · 85/0 (default) · clippy 0/0 ×2 · âncoras kittest
intactas. Binário instalado e rodando (boot validado no lab).

## T-890-UX4 — MESH COMO NO DESIGN + MÉTRICAS 42px (2026-10-07, `cd514e9`)

**Gatilho:** autor reprovou a UX3 mesmo com a autovalidação "confirmada" —
a análise automatizada por visão NÃO é juiz confiável (checklist satisfeito
≠ gestalt igual). Re-diagnóstico olhando EU MESMO close-ups do design:
(1) o mesh do design são **cards de assinante GRANDES e ricos** espalhados
e conectados ao hub — o meu era radial com cards 148×38 (canvas vazio);
(2) números do design ~44–52px, meus 32px; (3) canvas do design domina.

**Entrega:** `draw_mesh` refabricada — canvas 420px, cards 258px com faixa
superior colorida por tipo + título + 4 linhas de propriedades REAIS
(url/hb/lease, QoS do contrato, tópicos por papel, token), hub ESTAÇÃO
LOCAL, linhas 1.5px, clique→ASSINANTE SELECIONADO preservado; métricas
42px/h≥104. 101/0 (dds) · clippy 0/0. Comparativo: `logs/studio_ux4_vs_design_2026-10-07.png`.

**Lição registrada:** para fidelidade visual, o juiz é o olho humano
(autor) — automação serve para não-regredir, não para aprovar.
