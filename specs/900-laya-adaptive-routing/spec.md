# Especificação — Laya adaptive routing

Status: PROPOSTA M0 para revisão. Apenas a auditoria e a documentação foram autorizadas. Os comportamentos abaixo são requisitos futuros, não funcionalidades entregues. Autoridade: Prompt Master D1–D10 e plano M0 aprovado; diagnóstico em [audit.md](audit.md).

## Requisitos e aceites

| ID | Requisito | Given / When / Then | Marco |
|---|---|---|---|
| REQ-900-001 | Evidência e preservação | Dado HEAD identificado, quando auditado, então achados têm linha/commit, comandos têm resultados e hashes protegidos permanecem iguais. | M0 |
| REQ-900-002 | Fronteiras de execução | Dada intenção, quando classificada, então Python publica somente decisão; Rust controla validação, plano, tasks, agentes e políticas; integração interna usa DDS. | M1–M2 |
| REQ-900-003 | Contrato único e compatível | Dados bindings do mesmo IDL, quando intercambiadas amostras reais, então typename/key/XTypes/CDR/QoS coincidem e os 19 tópicos existentes continuam compatíveis. | M1 |
| REQ-900-004 | Decisão versionada não confiável | Dados enum inválido, NaN, revisão incorreta, baixa confiança ou expiração, quando validados, então não há execução induzida por esses dados e a causa é observável. | M2–M4 |
| REQ-900-005 | Laya como origem semântica de QoS | Dada decisão válida, quando aplicada, então objetivo Laya e QoS DDS efetivo são distintos na telemetria; NFCM não participa do novo caminho. | M4 |
| REQ-900-006 | Capabilities e autoridade | Dado agente incompatível/sem anúncio válido, quando observa Task adaptativa, então não faz claim; especialização, slots, saúde e política continuam necessários. | M3 |
| REQ-900-007 | Barreira e isolamento | Dada Task sem plano/requisitos verificáveis ou agente legado, quando chegam dados fora de ordem, então não há execução adaptativa até completar validação; legado não recebe essas tasks. | M3 |
| REQ-900-008 | Idempotência de decisão/materialização | Dados workers duplicados/replay, quando processados, então uma decisão efetiva persistida governa a revisão e uma tentativa usa sempre o mesmo task_id físico. | M2–M5 |
| REQ-900-009 | DAG validado | Dados ciclos, dependências/outputs ausentes, capability desconhecida ou limites excedidos, quando recebido plano candidato, então nenhuma Task é publicada. | M5 |
| REQ-900-010 | Execução e recuperação | Dados fork/join, falha e restart, quando retomado workflow, então joins aguardam outputs validados, tentativas antigas são ignoradas e estado deriva do journal. | M5 |
| REQ-900-011 | Segurança e efeitos externos | Dado prompt tentando elevar privilégios, quando classificado/planejado, então Policy Engine prevalece; efeito sem idempotência não é repetido automaticamente após resultado incerto. | M3–M5 |
| REQ-900-012 | Regressão acadêmica | Dado modo adaptativo desligado, quando executadas regressões, então prompts, drivers, semântica legada e dados congelados permanecem preservados. | M0–M7 |
| REQ-900-013 | Adoção gradual | Dados M1/M2, quando rodados, então não alteram Tasks; canary só inicia depois dos gates de interop, elegibilidade, QoS e recuperação aplicáveis. | M1–M6 |
| REQ-900-014 | Limites e observabilidade | Dado overload/timeout, quando recusado trabalho, então há causa tipada e uso limitado; logs não contêm prompt, credenciais ou cadeia de raciocínio. | M2–M6 |
| REQ-900-015 | Fallback determinístico | Dado Laya/planner indisponível, quando política decide, então rejeita ou usa template simples pré-autorizado sem ampliar permissões nem importar NFCM. | M2–M5 |
| REQ-900-016 | Retirada controlada NFCM | Dados dependentes inventariados, quando migrados em M7, então remoção só ocorre após regressões, compatibilidade CLI e rollback demonstrados. | M7 |

## Invariantes

- Semântica at-least-once. Readback, ownership, retry_count e journal não são transação de efeitos externos.
- Laya não escolhe identidades efêmeras como classes, não gera DAG e não autoriza ferramentas. Preservar distribuições e versões; não converter score em categoria por arredondamento.
- Versões/checkpoints/perguntas/catálogos/políticas fixados por intenção. Sem seleção silenciosa de revisão “mais recente”.
- Engine Rust é biblioteca de domínio + processo opt-in; não é scheduler central obrigatório dos experimentos antigos. Materializa readiness; agentes continuam fazendo claim.
- V1 adaptativa opera em domínio DDS dedicado com agentes atualizados e um engine ativo. Domínio é isolamento operacional, não controle de acesso criptográfico.
- Aplicação de QoS não modifica writer global por workflow; ownership strength continua determinado pelo papel.

## Limites propostos e medição

Bounds em [contracts.md](contracts.md) são limites iniciais de engenharia, não valores medidos nem metas de desempenho. Rejeitar acima do limite, nunca truncar silenciosamente. Prazo/custo são restrições superiores do ingresso autorizado; Laya só pode propor valores mais restritivos.

O limite de tokens gerados é global ao workflow, somando planner e tentativas de todos os nós. O orçamento monetário/custo inclui todas as operações cobradas, inclusive Laya. Reservar limites superiores transacionalmente antes de executar; forks/retries não renovam orçamento e consumo incerto esgota a reserva até reconciliação. Regra completa e unidades em contracts. Um plano com limites individuais válidos, mas soma acima do teto, é inválido (REQ-900-009/014).

Em M2, fixar dataset PT/EN rotulado, splits e baseline de regras antes da medição. Publicar macro-F1 por capability, erro por perfil, Brier/ECE quando aplicáveis e cobertura versus rejeição. Escolher thresholds em validação e congelá-los antes do teste separado. Sem threshold calibrado, permanecer shadow.

M6 separa custo de classificação, planner, DDS, claim e inferência; compara static/rules, Laya, planner e Laya+planner com manifests iguais no que for controlável. Metas numéricas, hardware/checkpoint e desenho amostral são decisões pré-campanha, não alegações M0. Estudo complementar; não declarar RQ5 aprovada.
