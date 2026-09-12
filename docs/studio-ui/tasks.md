# Studio UI — tasks (requisitos → fases → status evidenciado)

Convenção: cada tarefa cita UI-Rxx + UI-Gxx. Status exige evidência,
não existência de arquivo.

## UI-1 — Fundação visual

- [x] UI-1.1 Módulo `design` (tokens §7 dark/light; contraste primário ≥4,5 nos dois temas, teste verde). (UI-R05 · UI-G05/G06)
- [x] UI-1.2 `ApplicationShell`: barra superior (projeto, domínio, fonte, busca Ctrl+K) + temas; barra de operações recolhível pendente. (UI-R03/R04 · UI-G03/G04)
- [x] UI-1.3 Galeria de componentes com estados nos dois temas (`gallery_kittest.rs`: render + clique, sem pânico). (UI-R05 · UI-G05)
- [x] UI-1.4 Paleta de comandos (Enter nunca ativa perigoso; perigoso exige clique + revisão). (UI-R04 · UI-G03)

## UI-2 — Recursos e ambientes

- [x] UI-2.1 T01 Ambientes (recentes, demonstração rotulada) + T02 existente. (UI-R14 · UI-G11)
- [x] UI-2.2 T03 máquinas (`machines.rs` + `machines_kittest.rs`: saúde/confiança/DDS distintos, clique por apelido). (UI-R07 · UI-G10)
- [x] UI-2.3 T04 agentes: abas Definições × Instâncias, vínculo só comprovado ("Não confirmado"). (UI-R08 · UI-G14)
- [x] UI-2.4 T09 orquestrador-monitor (definição + papel "Supervisão…", aplicar indisponível G-INT-06). (UI-R12 · UI-G21)

## UI-3 — Editores e percursos

- [ ] UI-3.1 T05 editor de agente em 4 passos + resumo de destinos A/B/C. (UI-R09 · UI-G15/G18)
- [ ] UI-3.2 T06 ferramentas (gateway/host, restrição declarada × comprovada, sem executor inventado). (UI-R10 · UI-G22/G23/G24)
- [ ] UI-3.3 T07/T08 modelos × artefatos × cópias × servidores; caminho sempre com máquina. (UI-R11 · UI-G19/G20)
- [ ] UI-3.4 Pickers dependentes (host→dispositivo/caminho invalida com aviso localizado). (UI-R09 · UI-G17)

## UI-4 — Operações e colaboração

- [ ] UI-4.1 T10 workflows (sequência/fork-join/serial fiéis; intenção única por clique). (UI-R13 · UI-G25/G26)
- [ ] UI-4.2 T11 operações/logs (abas por tipo, scroll estável, sem confirmação inventada). (UI-R18 · UI-G29/G30/G31/G40)
- [ ] UI-4.3 T12 revisão/conflitos (base × local × remota; rascunho preservado). (UI-R15/R16 · UI-G28/G32/G33/G34)

## UI-5 — Integrações e preview

- [ ] UI-5.1 Modo demonstração rotulado + fixtures + cenários §20. (UI-R20 · UI-G43)
- [ ] UI-5.2 Matriz integração real × simulada × indisponível. (UI-R21 · UI-G42)
- [ ] UI-5.3 `integration-gaps.md` mantido (tela, entrada, dado externo, fallback). (UI-R28)

## UI-6 — Acessibilidade, performance, revisão

- [ ] UI-6.1 Teclado completo nos fluxos críticos + foco restaurado. (UI-R06 · UI-G07/G39)
- [ ] UI-6.2 Contraste verificado nos pares usados; relatório. (UI-R05 · UI-G06)
- [ ] UI-6.3 Zoom/escala nos cenários §8; capturas por cenário. (UI-R06 · UI-G08)
- [ ] UI-6.4 Orçamento de apresentação medido (sem alegar backend). (UI-R27 · UI-G46)
- [ ] UI-6.5 `evidence.md` final: matriz tela × estado × modo × integração. (UI-R26 · UI-G49/G50)
