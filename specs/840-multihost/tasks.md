# specs/840-multihost — Studio multi-host (MVP honesto)

**Gatilho:** o usuário provou os 4 hosts do laboratório (192.168.1.61 RTX 3080 / .62 orquestrador / .63 cliente / .64 agentes; Ubuntu 24.04, glibc 2.39, systemd --user ativo). Isso destrava parte dos 26 gates 🔒 da fase 800.

**Descoberta de deploy:** binários construídos localmente (glibc 2.43) não rodam nos hosts (2.39) — resolvido com **build musl estático** (`x86_64-unknown-linux-musl` + `+crt-static`; studio-node é Rust puro, sem C). Validado em host real antes do deploy.

| Task | Escopo | Estado |
|---|---|---|
| T-840-01 | studio-node: `STUDIO_NODE_BIND` (default 127.0.0.1) + `STUDIO_NODE_TOKEN` (Bearer, SHA-256 + comparação constante; `/version` aberto para sonda; **bind fora de localhost sem token ≥16 recusa o boot**); 6 testes de auth | [x] |
| T-840-02 | `scripts/deploy-studio-node.sh`: build musl, scp binário+unit, env com token (gerado e salvo SÓ localmente em `~/.config/studio/hosts/`), `pkill -x` de órfãos, daemon-reload/enable --now, linger best-effort, health check autenticado; `SSH_PASS` obrigatório via env (sem segredo no repo); unit `ExecStart=%h/dds-llm-rust/studio-noded` | [x] |
| T-840-03 | Máquinas no Studio: token nos clientes (`origin`/`catalog_remote`, `Debug` sem segredos, 401 tipado); `MachineRecord` no catálogo compartilhado (`kind:"machine"`, token NUNCA no catálogo — RNF-04); painel "Máquinas" (publicar com base condicional, probe por máquina com token em memória, dots online/offline/auth-pendente); 20 testes novos | [x] |
| T-840-04 | Deploy em .61/.62/.64 (substituindo os nós órfãos da era sem unit) + evidências multi-host (ver REPORT) | [x] |
| T-840-05 | REPORT + coverage complemento + notes.md + commits | [x] |

**Fora de escopo (segue para incremento futuro):** mDNS/descoberta de instalações (entrada manual via catálogo cobre o caso); 2ª GUI **gráfica** simultânea (evidência feita com cliente remoto via HTTP — mesmo fio); studio-storage/SQLite; executor de ferramentas ao vivo no mesh; auth do orquestrador HTTP.
