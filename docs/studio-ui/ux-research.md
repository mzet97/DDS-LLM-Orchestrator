# Studio UI — ux-research (síntese local das fontes do SDD)

Pesquisa documental prévia no SDD vigente (§3, R01–R17); sem entrevistas
nesta frente. Diferenças entre recomendação publicada e proposta local:

- Tabelas Carbon → `ResourceTable` egui com seleção por ID estável e
  virtualização; sem dependência web, só comportamento.
- Wizard PatternFly → assistentes de agente/servidor em 4 passos com
  revisão explícita; sem promessa de rollback de backend.
- Empty/loading Carbon → estados distintos (vazio real ≠ filtro vazio ≠
  acesso negado ≠ integração ausente); zero nunca é dado confirmado.
- WCAG 2.2 via WCAG2ICT → metas 4,5:1 texto, foco visível, teclado total,
  sem alegar conformidade integral; relatório em UI-6.
- egui_kittest → interação + snapshots por comportamento; compatibilidade
  0.36.2 verificada no lock.
- NN/G testes de usabilidade → roteiro de avaliação preparado em UI-6;
  screenshot ≠ teste com usuário.
