# Cobertura final de gates SDD — fase 800 (Studio local)

**Data:** 2026-09-11. **Evidência:** T-800-01…T-800-20 em `tasks.md` + testes
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
| G-34 | ◐ | restart adota sem migração destrutiva (local) |
| G-35 | ✅ | sem rollback prometido; tombstone explícito |
| G-36 | ◐ | journal/DB sem segredos; import não executa |
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
| G-58 | ◐ | journal persiste; sem eleição silenciosa |
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
