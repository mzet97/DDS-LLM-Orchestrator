# REPORT — specs/840-multihost (2026-10-05)

**Gatilho:** 4 hosts do laboratório disponibilizados (192.168.1.61 RTX 3080 / .62 orquestrador / .63 cliente / .64 agentes). Gates: fmt ✅ · clippy `-D warnings` ✅ · workspace **413+ testes, 0 falhas** (28 testes novos na fase).

## Evidências multi-host (ao vivo, 2026-10-05)

1. **Deploy reproducível em 3 hosts** (`scripts/deploy-studio-node.sh agent@192.168.1.{64,61,62}`): build musl estático → unit systemd --user + env (bind 0.0.0.0 + token) → health check autenticado OK nos 3. Substituiu os nós órfãos da era sem unit (binários de 12–14/09 fora do git, protocolo 1.0, sem persistência).
2. **Auth (G-44):** no .64 — `/operations` sem token **401**, token errado **401**, token correto **200**, `/version` aberto (`{"major":1,"minor":0}`). Comparação em tempo constante sobre SHA-256.
3. **Catálogo compartilhado cross-host (G-70 parcial):** cliente na **.62** publicou `machine:orquestrador-62` no nó da **.64**; a máquina local leu o **snapshot** com o item e os **eventos** (`created`, seq 0) — três hosts, uma autoridade, mesmo fio que a GUI consome (T-830-02).
4. **Actuação remota (.61):** `POST /services/dds-agent/start` → `wanted:true, active:true` em 1 s (agente real subiu: "claim loop iniciado", domínio 42); `stop` → `wanted:false` e serviço `inactive` confirmado no host. Idempotência (`already_applied`) e rejeição de payload malformado (400) também observadas.
5. **GUI:** painel "Máquinas" publica/sonda máquinas do catálogo compartilhado com token (20 testes de fio/unit; token só em memória, `Debug` redigido).

## Correções de deploy descobertas no caminho

- **glibc:** binários locais (2.43) não rodam nos hosts (2.39) → musl estático (studio-node não tem deps C).
- **Unit stale:** `studio-noded.service` apontava `%h/.local/bin/studio-noded` (era T-800-15) → `%h/dds-llm-rust/studio-noded` (consistente com os demais binários e com o script).
- **`%h` em EnvironmentFile não expande** (T-830-05) — env.example já corrigido; persistência agora é default do binário.
- **Órfãos:** .61/.62 tinham studio-noded rodando **sem unit e sem linger** (morte no logout; binário fora do git) — substituídos por instâncias sob unit com `enable --now`.
- **Password never in repo:** script exige `SSH_PASS` por env; tokens gerados ficam em `~/.config/studio/hosts/<host>.token` (fora do repositório).

## Estado dos gates da fase 800 após 840

- G-44 (auth do catálogo): ◐ → **✅ local/LAN** (token Bearer; eleição/replica seguem n/a).
- G-70 (duas instalações): 🔒 → **◐** (2ª instalação provada por cliente HTTP remoto; 2ª GUI gráfica simultânea segue pendente).
- G-02 (cadastro de máquinas remoto): 🔒 → **◐** (cadastro via catálogo + deploy script; fingerprint/host-key gerenciado segue básico).
- G-15 (criação de unidades remota): parcial — unidade `dds-agent` instalada por provisionamento manual+API; criação de unidades NOVAS pela GUI segue pendente.
- **Seguem 🔒:** mDNS (G-41/42), ferramentas ao vivo (G-18..22), wf-run pela GUI (G-25/26), modo protegido (G-38/65), egui_kittest (G-37), verificação GGUF×manifesto (G-10/11), macOS/Windows.

## Pendências conhecidas

- Atuação remota: janela de transição no probe pós-stop (leitura imediata pode apanhar `active:true` em desligamento) — cosmética; confirmado `inactive` no host.
- `.63` (cliente) não precisa de nó; ficou sem deploy.
- Próximos incrementos naturais: mDNS, deploy do **agent** atual (musl com CycloneDDS C — requer validação de cross-build) em .64 para EXP-C, e campanha EXP0–EXP4 (o caminho crítico da dissertação).
