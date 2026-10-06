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
| G-40 | ◐ | `studio-noded` CLI + `.desktop` validado + unit |
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
G-33, G-37 (egui_kittest), G-38, G-41, G-53, G-60, G-61, G-64, G-65,
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
  G-25/26 (wf-run GUI), G-37/38/65, G-10/11.
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
