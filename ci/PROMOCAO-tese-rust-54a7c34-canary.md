# Registro de promoção — canário operacional do studio-noded (2026-09-14)

**Finalidade: APROVADO PARA CANÁRIO DE ATUALIZAÇÃO do daemon
studio-noded EXCLUSIVAMENTE no host 192.168.1.62.** Não aprovado para
outros hosts, agentes, orquestradores ou inferência. O registro anterior
(ci/PROMOCAO-tese-rust-54a7c34.md) permanece válido para ensaio isolado.

| Campo | Valor |
|---|---|
| Artefato (idêntico ao ensaio — NÃO reconstruído) | `harbor.home.arpa/tese/tese-rust@sha256:ca4836d7831fb07fb6fb869439f62d52ca172c763169226c2f2e068892382a76` |
| Revisão-fonte | `54a7c349973b012d05a8ad277133fcb21cba9ac1` |
| SHA-256 do studio-noded aprovado | `9a3a1f9fbe1d07f6fc3275cc955785f7beed75879978c56e2e84de76b56028e0` |
| Alvo | 192.168.1.62, unidade `studio-noded.service` (enabled, User=agent, WorkingDirectory=/home/agent/dds-llm-rust, bind 192.168.1.62:4317, DB=studio-node-log.json, sem drop-ins) |
| Instalação anterior | sha256 `9a5ab50bf1059c7774be0bc457059c9b89d2eef5fd62ffdc06e9701de428efb9` (build 2026-09-12 13:27, sem registro de revisão/procedência CI) |
| Diferença identificada | revisões distintas; candidato é build posterior (2026-09-14, CI run nº8 completa) — recência por EVIDÊNCIA de data, não suposição |
| Estado a migrar | nenhum DB existente (nasce na 1ª operação); compat de leitura/escrita demonstrada em ambiente separado (ensaio F3 + teste compat 2026-09-14: /version 1.0, /services, /apply applied) |
| Retorno | binário anterior + unit preservados em `releases/<run>/`; rollback = restaurar binário anterior + restart (esquema de DB idêntico entre builds) |
| Limite de indisponibilidade | 90s (stop→swap→start→prontidão) |
| Critérios objetivos de rollback | R1 exe do PID ≠ aprovado; R2 /version ≠ 1.0; R3 listener 4317 ≠ MainPID; R4 qualquer PID de agente/inferência/orquestrador alterado; R5 unidade inativa após start+20s |

Isolamento de parada demonstrado (preflight 2026-09-14): cgroup da
unidade contém SOMENTE o MainPID; Requires/Wants/PartOf/TriggeredBy não
referenciam agentes/inferência.
