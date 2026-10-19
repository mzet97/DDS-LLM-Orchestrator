# Riscos, decisões para revisão e bloqueios

M0 pode entregar documentação com limitações explícitas; isso não libera o gate funcional dos próximos marcos. Dono técnico = IA executora; decisões de compatibilidade/metodologia = usuário/arquiteto, e orientador quando acadêmicas.

| ID | Risco / estado | Mitigação ou decisão proposta | Dono / marco bloqueado |
|---|---|---|---|
| D-QOS | PROPOSTA: QoS de aplicação pode ser insuficiente para uma expectativa de QoS nativo variável | Aprovar ADR-002 ou exigir design de endpoints antes de M4; não chamar prioridade de aplicação de Reliability DDS por amostra. | Usuário/arquiteto; M4 |
| D-ISOLATION | PROPOSTA: coexistência de agentes antigos burla capabilities | Aprovar domínio dedicado + agentes atualizados; resolver como backend DDS participará dessa implantação. Mesmo domínio misto não é alternativa autorizada. | Usuário/arquiteto; M3 |
| D-JOURNAL | PROPOSTA: autoridade duplicada, perda/replay incorreto | Aprovar ADR-004: DB SQLite local, um engine, outbox; sem HA multi-host e sem WAL em CIFS. | Usuário/arquiteto; M5 |
| R-INTEROP | RESOLVIDO PARA O GATE M1 (2026-10-17): interop real Rust↔Python 4/4 (V02/V03) — o mismatch de bound de string NÃO impediu matching (build.rs anuncia TypeIds do idlc C). RESÍDUO: lado Rust não valida bounds de string localmente (validação de aplicação em M2/M3); colisão de literais de enum seguida por emenda de prefixos (M1-01) | Validar bounds no intake/validator (M2/M3); harness reutilizável em workers/laya-worker/harness | Executor; M2/M3 |
| R-PYGEN | NÃO RESOLVIDO (M1-01): wheel cyclonedds 11.0.1 (py3.14) não traz backend `_idlpy` (`idlc -l py` falha e chega a exit 0 sem gerar); sdist 11.0.1 só compila `_idlpy` com libcycloneddsidl em `lib/` (instalação local usa `lib64/`) e seu `annotations.py` quebra no import em py3.14 (PEP 649) | Receita validada em venv scratch py3.13 + CYCLONEDDS_HOME sombra (evidence/m1-01/README.md §Reprodução); M1-02 fixa a receita/pin no worker (pyproject) ou adota geração alternativa; nada instalado no ambiente do usuário | Executor; M1-02 |
| R-LAYA | **RESOLVIDO (2026-10-17)**: laya==0.4.10 (Apache-2.0) em venv dedicada; checkpoint pinado (sha256 891102d3…, verificado no load); torch CPU nesta máquina | RESÍDUO: checkpoint com temperaturas inválidas (confiança não calibrada → shadow); dataset seed não é calibration-grade; campanha N≥30 e braços planner E2E pendentes | Executor; M6 completo |
| R-CALIBRATION | PARCIALMENTE VERIFICADO: avaliação real no seed (regras > Laya no seed — reportado); confiança média Laya 0.169 (não calibrada, aviso do SDK) | Dataset rotulado adequado + recalibração do checkpoint antes de qualquer gate de confiança; shadow segue | Responsável experimental; M6 completo |
| R-PDF | BLOCKED para conferência primária: PDF exato não localizado | Identificar a versão correta; usar consolidação apenas como fonte indireta e preservar diferenças acadêmicas. | Usuário fornece/localiza fonte; revisão acadêmica |
| R-SECURITY | NÃO VERIFICADO: autenticação DDS neste ambiente | Reutilizar policies existentes, validar identidade e permission sets antes de rede não confiável. Capability/requester declarados não autenticam emissor. | Executor; implantação não confiável |
| R-EFFECTS | Side effect pode ocorrer antes da confirmação | Idempotency key específica do handler; IN_DOUBT sem retry automático; TOOL desabilitado até gate V10. | Executor; M5 |
| R-LEGACY | CONFIRMADO: tipos/StaticDecider e benchmarks dependem de qos-nfcm | Não apagar crate; inventariar consumidores e separar contratos neutros em M7, mantendo artefatos reproduzíveis. | Executor; M7 |
| R-SNAPSHOT | HEAD e ambiente podem mudar antes da outra IA | Conferir SHA/status/hashes; se mudou, atualizar somente evidência afetada, sem tratar este relatório como verificação do novo HEAD. | Próxima IA; início de todo marco |
| R-TOOLS | RTK/Graphify indicados ausentes | Inventário por símbolos/manifestos; registrar limitação, não inventar instruções nem instalar ferramentas neste M0. | Executor; sem bloqueio documental |
| R-BASELINE | Baseline inicial teve falha induzida pelo namespace root/capabilities | Log original preservado, teste exato reexecutado sem capabilities e baseline repetida; resultados finais em REPORT. | Executor; avaliação da baseline |
| R-BOUNDS | Bounds novos podem exceder middleware ou custo aceitável | Limites são proposta, não medições; M1 testa máximos e M6 mede recursos; congelar revisão antes de usar. | Executor; M1/M6 |
| R-ORDER | Ordering inter-tópicos e respostas tardias | Barreira Plan+State+Task, revalidação pós-claim, task_id distinto por tentativa, expirations. | Executor; M3/M5 |
| R-SINGLE | Engine único limita disponibilidade | Declarar sem HA V1; lock local não resolve split-brain entre hosts; ADR separado para HA. | Arquiteto; extensão além da V1 |

## Regra de revisão

As decisões D-QOS/D-ISOLATION/D-JOURNAL e o contrato M0-1 são propostas concretas para revisão, não escolhas já aceitas por aprovação genérica do plano de auditoria. A próxima IA pode executar M1 se a sessão de destino autorizar expressamente esses contratos; não solicitar novamente decisões fechadas D1–D10 do Prompt Master. Na ausência dessa autorização, apresentar os pontos específicos e continuar apenas trabalho documental/read-only.

## Rollback e preservação

M0 é reversível por remoção apenas dos arquivos novos deste dossiê, se solicitada; não executada automaticamente. Nenhum reset/stash/rebase/commit é necessário. Para M1–M7, rollback segue plan/ADRs: suspender admissão, conservar journal/outbox, reconciliar trabalho em voo, desligar componentes adaptativos. Não copiar tasks adaptativas para domínio legado nem apagar efeitos/resultados para simular reversão.

Os hashes externos cobrem o corpus inventariado, não todos os arquivos ignorados do projeto pai. Campanhas anteriores não foram reexecutadas, aprovadas metodologicamente nem reescritas nesta auditoria.
