# Studio UI — design (navegação, tokens, componentes, decisões)

## Navegação (máx. dois níveis, §6)

```
Ambiente / projeto atual                     Busca e comandos (⌘K/Ctrl+K)

Visão geral

RECURSOS
  Agentes            (= definições; instâncias em aba)
  Ferramentas
  Workflows

INFRAESTRUTURA
  Máquinas e rede
  Inferência
    Modelos          (= modelo lógico × artefato × cópias)
    Servidores llama.cpp
  Orquestradores     (= monitor; rótulo "Supervisão de tarefas e agentes")

ATIVIDADE
  Operações e execuções

SSH dedicado                                   (capacidade existente G-04)
Catálogo / Nó / Despacho / Serviços / Topologia (telas existentes preservadas)

Configurações                                  fixo no fim
```

Ajuste da árvore existente (não segunda árvore): renomear/agrupar seções
atuais para esta taxonomia; "Modelos GGUF" vira "Inferência › Modelos";
"Subir inferência" vira assistente dentro de Servidores.

## Tokens (§7, pares a verificar em UI-6.2)

`background/surface/surface_raised`, `text_primary/secondary`,
`border_subtle/border_control`, `accent/on_accent`,
`success_text/warning_text/danger_text` — cada um com par dark/light da
tabela do SDD. Tema: Sistema / Claro / Escuro (§13 T13). Tipografia:
corpo 15, rótulo 14, metadados 12–13, código 13–14, título 24, seção 18;
monoespaçada só para IDs/caminhos/args/logs. Espaçamento 4/8/12/16/24/32;
controles h36, alvo icônico ≥32, linhas 40/32; raio 6–8.

## Componentes (ordem de construção)

1. `StatusBadge` (texto+símbolo+cor), `FreshnessIndicator` (fonte+idade),
   `CapabilityNotice` (motivo da ausência), `EmptyState`/`ErrorState`.
2. `PageHeader`, `ResourceTable` (busca/filtro/ordenação/seleção por ID),
   `Field`/`FieldGroup`, `SecretField` (definido; manter/substituir/remover).
3. `SearchablePicker` → `HostPicker`/`ModelArtifactPicker`/`DevicePicker`.
4. `DefinitionEditor` (base × rascunho × diff), `ReviewPlan`, `ConflictResolver`.
5. `OperationPanel`, `LogViewer` (pausa/pesquisa/limite), `JsonInspector`,
   `ConfirmationDialog` (recurso+host+impacto+ação nominal),
   `CommandPalette`, `NotificationCenter`, `WorkflowView`/`TopologyView`
   (sempre com alternativa tabular).

## Decisões de interação

- Intenções tipadas (`UiIntent`) com contexto de projeto; sem efeito remoto
  em renderização; deduplicação por gesto (UI-G26).
- Dimensões de estado visíveis (§13): contexto, origem, confiança/acesso,
  gerenciamento, configuração, ciclo observado, atualidade, operação —
  nomes de apresentação, sem tocar enums da IDL.
- Demonstração sempre rotulada "Demonstração · dados simulados"; cache
  separado do real; fechar janela não emite stop remoto (UI-G45).
- Microcopy da tabela §16 (capacidade ausente, recurso externo, stale,
  publicação aceita, conflito, demonstração, sem confirmação).
