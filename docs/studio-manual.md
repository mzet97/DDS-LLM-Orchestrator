# Manual do Usuário — DDS Orchestrator Studio

> Aplicativo: `orchestrator-studio` (binário `studio`) + `studio-node` (binário
> `studio-noded`). Referência técnica:
> [crates/orchestrator-studio.md](crates/orchestrator-studio.md),
> [crates/studio-node.md](crates/studio-node.md).
> Telas marcadas **[requer `dds`]** só existem no build completo.

O Studio é o desktop de operação do laboratório DDS: descobrir nós e agentes,
conversar com a inferência, despachar tarefas, rodar o workflow A→B→C, governar
o catálogo compartilhado, atuar serviços systemd remotos, auditar modelos GGUF
e observar o barramento — tudo somente-leitura real, sem dados fictícios.

## 1. Instalação e primeira execução

```bash
cd src/rust
cargo run -p studio-node --bin studio-noded            # 1) nó em cada host (127.0.0.1:4317)
cargo run -p orchestrator-studio --bin studio --features dds   # 2) GUI completa
```

Sem `--features dds`, a GUI abre em modo HTTP-only (sem Topologia, Ferramentas,
corredor DDS do Workflow e worker de descoberta). Pré-requisitos: Rust ≥ 1.85,
display gráfico; com `dds`, CycloneDDS (+ `CYCLONEDDS_STATIC=1` em SMB/CIFS) e
domínio coerente (`STUDIO_DDS_DOMAIN` = `STUDIO_NODE_DDS_DOMAIN`, default 170).

No boot, confira no header o alvo único (pill `NÓ ALVO`) e a descoberta em
**3.10 Máquinas**. Com Auto-Carga ligada, a GUI lê Nó+Serviços+Catálogo e
re-lê a cada 5 s.

## 2. Conceitos

- **Nó alvo**: o `studio-noded` selecionado (único por vez). Tudo que é "do nó"
  (serviços, catálogo, operações) refere-se a ele.
- **Descoberta**: ouvintes de `Studio.NodePresence` (heartbeat 5 s, lease 10 s)
  + `AgentRegistry` + `ServerStatus`. Nós entram como DESCOBERTO; o primeiro
  online pode ser auto-selecionado.
- **Modo protegido**: default DESARMADO (leitura somente). Ações com efeito real
  (start/stop) exigem armar + confirmação em 2 passos por ação.
- **Segredos**: tokens vivem SÓ em memória (nunca em disco; registros de máquina
  não serializam token por construção).
- **Honestidade**: cada card/tela marca `stale`/vazio quando a fonte nunca foi
  lida; erros de fio (401, offline, 409) aparecem como avisos, nunca como zero
  silencioso.

## 3. Tour pelas telas

### 3.1 Visão Geral
Agregado somente-leitura: 6 cards (nó, serviços, agentes, modelos,
inferência/prova, catálogo) + descoberta. Atalhos `+`/`-`/`0` ajustam escala e
persistem. Cards com fonte nunca lida exibem estado stale explícito (não falha).

### 3.2 Nó studio-node
Conexão RPC ao alvo: `GET /version` + `GET /operations`, token em memória, RTT
medido com média móvel, 3 cards (protocolo · operações · lease 10 s), log de
operações com filtro + inspetor JSON e card que distingue falha 401 de falha de
rede. Use **Re-probe** para medir o RTT de novo.

### 3.3 Inferência & Chat **[chat]** 
Chat de engenharia OpenAI-compatível: descoberta `ServerStatus`, endpoint HTTP +
probe `GET /v1/models`, parâmetros (temperatura, top_p, max tokens — vão no corpo
do POST), transcript com estatísticas reais do turno e composer com Ctrl+Enter.
Fluxo verificado (QA F2): enviar → resposta → transcript exportável.

### 3.4 Subir Inferência
Runner **local** de `llama-server` (subprocesso direto, sem systemd): presets
Precisão/Latência/Agente, prévia da CLI, PID/status/VRAM (hwmon AMD)/uptime
reais, wizard em 5 etapas com tempos medidos, console de 500 linhas, chip
ServerStatus, parada SIGTERM→SIGKILL. Sem modelo configurado, a validação falha
com erro honesto, sem crash (QA F13).

### 3.5 Agentes
Duas fontes distintas, lado a lado: FONTE 1 = DDS `AgentRegistry` ao vivo;
FONTE 2 = HTTP auxiliar `GET /api/v1/agents` (URL editável, default
`http://192.168.1.61:8080`). Seção A (slots/lease), Seção B (taxas reais do
orquestrador), agregado do enxame e snapshot JSON exportável. Fluxo verificado
(QA F8): apontar a URL → Atualizar → tabela populada + agregado; backend morto
gera banner honesto "Orquestrador HTTP inacessível" (QA A3).

### 3.6 Despacho
Teste síncrono direto `POST /api/v1/chat/completions/sync`: parâmetros à
esquerda, card de resultado com decomposição T1–T6 + abas payload bruto/headers
e histórico da sessão. Sem stop/tok-s (não existem no contrato). Fluxo verificado
(QA F3): payload exato no servidor + status final.

### 3.7 Modelos GGUF
Inventário local SHA-256: diretório (`STUDIO_MODELS_DIR` ou `$HOME/tese/models`)
+ manifesto congelado `{"models":{…}}`, worker de hash com progresso e
cancelamento, 4 cards resumo, filtros por status (Pendente/OK/DESVIADO/sem
registro), tabela calculado×manifesto e export de relatório. Fluxo verificado
(QA F6): inventariar → hash idêntico ao `sha256sum` → manifesto OK; hash
adulterado → DESVIADO.

### 3.8 Serviços (systemd)
Daemon control no alvo: lista `GET /services` (pretendido × efetivo
`{service,wanted,active}`), intertravamento do modo protegido + confirmação em
2 passos, `start`/`stop` (+ `restart` = stop→start) e auditoria com relógio.
Fluxo verificado ponta a ponta (QA F14): expandir plano → ARM → Iniciar +
Confirmar → `active:true` real no systemd e operação registrada; Parar +
Confirmar → `inactive`; com DISARM, a ação é recusada com aviso e nada trafega.

### 3.9 Catálogo Compartilhado
Cliente da autoridade OCC do nó: snapshot/eventos, banner 409 estruturado (base
vs vigente), 4 cards, gaveta de publicação com write-lock (ID, KIND/partição,
base revision, valor JSON com validação de sintaxe), busca+filtros e feed de
mutações. Fluxos verificados (QA F5): publicar → item `r0` no snapshot;
tombstone → item some e cursor avança; base obsoleta → 409 sem mutação (QA A2).
KIND deriva do prefixo do ID.

### 3.10 Máquinas
Descoberta + registro multi-host: participantes/varredura, tabela canônica de
`Studio.NodePresence`, janela de lease 10 s, classificador de falha
(online/offline/`auth-pending` 401), mini topologia e registro manual
(**Testar & Registrar** → `machine:<id>` no catálogo; token digitado fica só em
memória). Fluxos verificados (QA F9/A7): URL morta → "probe não passou; registro
NÃO persistido" e nada é gravado; nós do laboratório aparecem FRESCO via DDS
(QA V1).

### 3.11 Topologia DDS **[requer `dds`]**
Mesh desenhado (cards clicáveis → assinante selecionado), faixa de 6 contadores,
chips de filtro por tópico canônico e 7 abas (Tarefas/Agentes/ServerStatus/
ToolCalls/Métricas/Descoberta/Instalações) + log `TaskOutput` no rodapé. Janela
1–30 s (default 5 s). Só `read`, nunca `take`. Tela inicial do modo completo.
Fluxo verificado (QA F11): leitura do domínio com contadores reais.

### 3.12 Workflow A→B→C
Pipeline canônico: formulário (domínio/entry/modelo/timeout), `start()` injeta o
corredor que replica `wf-run run_seq` (prompts congelados), 3 cards de estágio,
auditoria e painel de exceção com o erro real. Sem `dds`: máquina de estado
testável + orientação. Fluxo verificado (QA F1): 3/3 `done` em ~6 s com
cross-confirmação na 3.6.

### 3.13 Ferramentas & Tool Calls **[requer `dds`]**
Governança ao vivo de `ToolCall.Request`: cards N0/N1/N2, chips por status
canônico + REQ/s + bloqueios, tabela requester/nível/duração e inspetor
REQUEST×RESPONSE (payloads íntegros, cap 8 KB). O resultado volta na MESMA
instância (sem tópico Response). Fluxo verificado (QA F4): montar chamada +
export JSON.

### 3.14 Logs da GUI
FIFO de 500 entradas: faixa stream (taxa honesta), 4 KPIs, filtros por nível,
busca, auto-scroll, espelho stderr, pausa com foto congelada, limpeza e export
`.log`, tabela invertida + inspetor forense. Fluxo verificado (QA F10): export
não-vazio + limpar esvazia.

## 4. Fluxos guiados

**Conectar ao laboratório.** Suba `studio-noded` nos hosts (com
`STUDIO_NODE_DDS_DOMAIN` igual ao da GUI e token ≥ 16 chars se expor na LAN).
Abra a GUI, confira os nós FRESCO na 3.10 e o alvo no header.

**Registrar uma máquina.** 3.10 → Registro Manual → nome/URL/token → Testar &
Registrar. A chave é validada por probe imediato em `/version` antes de
persistir; falha de probe não persiste nada.

**Publicar no catálogo.** 3.9 → gaveta → ID + valor JSON (base vazia = criação)
→ Publicar Registro. Conflito de revisão retorna 409 com a vigente (sem aplicar).

**Atuar um serviço.** 3.8 → expandir plano → ARM → Iniciar/Parar/Reiniciar →
Confirmar. Cada ação gera `operation_id` auditável; DISARM bloqueia tudo.

**Rodar o workflow.** 3.12 → preencher → Executar → acompanhar os 3 estágios e
a auditoria; exceção mostra o erro real.

**Auditar modelos.** 3.7 → Inventariar → conferir OK/DESVIADO → Exportar relatório.

## 5. Variáveis de ambiente (GUI)

`STUDIO_SCREEN` (tela inicial 3.1…3.14), `STUDIO_DDS_DOMAIN` (default 170),
`STUDIO_TARGET_URL` (força o alvo), `STUDIO_SCROLL_Y`, `STUDIO_WIN_H`,
`STUDIO_ONTOP=1`, `STUDIO_MODELS_DIR`, `STUDIO_GIT_HASH` (build). O binário não
aceita argumentos CLI. Nó: `STUDIO_NODE_*` (porta, bind, token, serviços, DB,
domínio DDS, URL pública, ID) — ver [studio-node](crates/studio-node.md).

## 6. Solução de problemas

| Sintoma | Causa provável | Ação |
|---|---|---|
| 3.11 mostra aviso / 3.13 ausente | Build sem `--features dds` | Recompilar com a feature |
| Card "Falha Auth (401)" / `unauthorized` | Nó com token, GUI sem token em memória | Informar o token (3.2/3.10); ele não é gravado |
| "Timeout / Porta Recusada" | Nó fora do ar ou firewall | Checar processo/porta 4317; consulta `GET /version` |
| "Nenhum agente listado" (3.5-B) | URL HTTP errada ou orquestrador parado | Conferir URL; Seção A (DDS) é a fonte primária |
| Publish retorna 409 | Base obsoleta | Re-ler o snapshot e republicar com a base vigente |
| Catálogo vazio após tombstone | Comportamento correto | `cursor` avançou; eventos registram o `deleted` |
| Botão parece não responder | Layout reflui com conteúdo vivo; medida→clique deve ser rápida | Re-mirar pelo estado atual da tela |

## 7. Limites conhecidos

Sem `dds` ≠ completo (selo por tela acima). Sem GUID de servidor/nó, sem métricas
de inferência além do fio (3.3), runner local sem systemd (3.4), sem p95/jitter
(3.5), sem BLAKE3 nem tópico de verificação (3.7), sem PID/subestado (3.8), sem
SSE (3.9), política estática/permissiva M2 sem RBAC (3.13). Payloads DDS com cap
8 KB; console/logs com cap 500. Textos dos gates G-38/65 aguardam validação
contra o SDD. Evidências do QA funcional: matriz round 1 (`manual-qa.md`) e
round 2 (`manual-qa2.md`) nos artefatos de teste.
