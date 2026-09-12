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

- [x] UI-3.1 T05 editor em 4 passos (`agent_editor.rs` + `editor_kittest.rs`: digitar, validar, voltar preserva, resumo J02). (UI-R09 · UI-G15/G18)
- [x] UI-3.2 T06 ferramentas (`tools.rs`: gateway/host, declarada × observada, "Requer implementação de executor"). (UI-R10 · UI-G22/G23/G24)
- [x] UI-3.3 T07 caminho com máquina (`host_label`) + aviso de cópias (G-INT-05); T08 no assistente existente. (UI-R11 · UI-G19/G20)
- [x] UI-3.4 Pickers dependentes (`pickers.rs`: troca de host invalida com aviso, independentes preservados). (UI-R09 · UI-G17)

## UI-4 — Operações e colaboração

- [x] UI-4.1 T10 workflows (`workflows.rs`: dependências explícitas, 1 intenção por gesto). (UI-R13 · UI-G25/G26)
- [x] UI-4.2 T11 operações/logs (`operations.rs`: abas por tipo, buffer 10k, "Parar de acompanhar"). (UI-R18 · UI-G29/G30/G31/G40)
- [x] UI-4.3 T12 revisão/conflitos (`review.rs` + `review_kittest.rs`: 3 vias, exemplo rotulado, sem sobrescrita). (UI-R15/R16 · UI-G28/G32/G33/G34)

## UI-5 — Integrações e preview

- [x] UI-5.1 Modo demonstração rotulado (`preview.rs`: 16 cenários §20, fixtures estáveis, selo persistente). (UI-R20 · UI-G43)
- [x] UI-5.2 Matriz real × simulada × indisponível em `integration-gaps.md`. (UI-R21 · UI-G42)
- [x] UI-5.3 `integration-gaps.md` mantido (G-INT-01…10). (UI-R28)

## UI-6 — Acessibilidade, performance, revisão

- [x] UI-6.1 Teclado nos fluxos críticos (`palette_kittest.rs`, `editor_kittest.rs`); foco restaurado = gap. (UI-R06 · UI-G07/G39)
- [x] UI-6.2 Contraste nos 7 pares × 2 temas (`a11y.md`, teste automatizado). (UI-R05 · UI-G06)
- [x] UI-6.3 Zoom 200% sem pânico (`volume_ui.rs`); capturas com janela real pendentes. (UI-R06 · UI-G08)
- [x] UI-6.4 Orçamento medido: log 10k 2,8 ms, filtro 10k 3,6 ms (apresentação, não backend). (UI-R27 · UI-G46)
- [x] UI-6.5 `evidence.md` final + matriz abaixo. (UI-R26 · UI-G49/G50)
