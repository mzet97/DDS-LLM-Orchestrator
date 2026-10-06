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
