# Plano de validação

Status: baseline M0 executada conforme [REPORT](REPORT.md); cenários V02–V12 são testes FUTUROS, não resultados. Cada cenário usa Given/When/Then abaixo. REQ e marcos em [traceability](traceability.md).

## Baseline M0 e ambiente

Inspecionados usos de arquivos/processos/rede dos testes: fixtures de context-store/MCP/Studio usam diretórios temporários; serviços HTTP/SSH de testes usam loopback; inferência real do agente é `#[ignore]`. Não invocar `--ignored`, bins de benchmark, scripts de campanha ou serviços de produção.

Target local exclusivo; `CYCLONEDDS_STATIC=1`, lockfile preservado, quatro jobs de build, debug info desabilitado para artefatos de teste, cargo offline, uma thread de teste por harness. Cada comando DDS em namespace de usuário/rede novo, loopback e peers locais, sem multicast e sem rota externa. Domínios fixos dos testes permanecem isolados do host. Metadados completos nos JSON de evidence.

Comandos: `cargo test --workspace --locked`; `cargo clippy --workspace --locked -- -D warnings`; `cargo fmt --all -- --check`; teste separado de dds-contract sem feature (mock); testes de dds-contract/dds-dataspace/agent/orchestrator com `--features dds --locked -- --test-threads=1`. Teste focado não substitui baseline do workspace. Resultados ignorados, filtrados e binários não atingidos precisam ser discriminados.

Primeira execução em namespace mapeado como root reteve capabilities e invalidou a expectativa de permissão 0444 de um teste. Manter esse log. Validar hipótese com teste exato após remover capabilities (`setpriv --bounding-set=-all`) antes do processo Cargo; só depois repetir baseline. Não alterar o teste para contornar o resultado.

## Cenários e superfície observável

| ID | Given / When / Then | Superfície / evidência |
|---|---|---|
| V01 | Dado snapshot protegido, quando M0 termina, então hashes do corpus conferem, diff funcional é vazio e links/linhas existem. | before/after, Git, verificador documental, revisão independente. |
| V02 | Dados publishers Rust/Python reais do mesmo IDL, quando trocam tipos nas duas direções, então conteúdo, keys, typename, TypeIds/CDR e limites conferem. | Dois processos CycloneDDS, logs de matching e bytes/fixtures; incompatibilidade deliberada deve falhar. |
| V03 | Dado subscriber tardio/reiniciado, quando recebe dados retidos ou publishers morrem, então observa a semântica de durabilidade/expiração especificada; perda total de writers não se confunde com journal. | DDS real, restart, resource limits, max payload e hashes. |
| V04 | Dados PT/EN, negação, injeção, opções reordenadas, baixa confiança, NaN, erro e timeout, quando Laya é observado, então decisão correlacionada/versionada e zero Tasks publicadas; sem pesos em unitário. | Python fake DDS/Laya + observador real shadow; dataset/calibration separados. |
| V05 | Dado agente sem capability/saúde/slots/modelo/política ou target diferente, quando tenta claim com prefixo vazio, então não executa no modo adaptativo. | Fixture in-memory para regras; agente DDS real para confirmação/readback; ausência de agente elegível gera causa. |
| V06 | Dadas Tasks antes/depois de Plan/State/Capabilities e reinício de subscriber, quando reavaliadas, então só executam com requisitos completos, revisões exatas e tentativa ativa. | Permutações determinísticas + DDS real; cache incompleto expira sem execução. |
| V07 | Dados workflows simultâneos com QoS distintos, quando executam, então QoS DDS estrutural/strength não muda globalmente e objetivos/fallback são observáveis. | Inspeção de endpoints + aplicação; mismatch estrutural deliberado recusa. |
| V08 | Dado DAG com ciclo, nó ausente/duplicado, output impossível, capacidade desconhecida, modelo indisponível, prazo vencido ou limites excedidos, quando validado, então zero Tasks; DAG válido respeita joins. | Unit/property de grafos + integração in-memory/DDS, equivalência seq/fork-join. |
| V09 | Dados K=2/K=4 agentes, decisões duplicadas, restart e falhas após commit/publicação/ASSIGNED/RUNNING/chunk final, quando reconciliados, então slot de decisão e tentativa ativa não mudam por replay e saída velha não libera join. | Failpoints, journal/outbox, contagem de tentativas/efeitos; não inferir probabilidade nula de duplicação. |
| V10 | Dado efeito não idempotente sem conclusão persistida ou cancelamento em voo, quando reinicia, então IN_DOUBT/reconciliação e nenhum retry automático inseguro. | Handler instrumentado, policy real, falha antes/depois do efeito; negar elevação de permissão por prompt. |
| V11 | Dado canary com entrada/backend/carga congelados, quando comparadas políticas, então métricas separam Laya/planner/DDS/claim/inferência e reportam sucesso/erro/recuperação/recursos. | Manifest por run independente, warmup/outliers pré-definidos, P50/P95/P99, F1/ECE/Brier quando aplicáveis; nenhum resultado presumido. |
| V12 | Dado NFCM isolado em M7 e adaptativo desligado, quando rodam regressões legadas, então CLI, seq/fork-join, EXP1b/EXP4 e artefatos permanecem reproduzíveis. | Inventário de dependentes, diffs/hashes e testes antigos preservados; sem rerun acadêmico indiscriminado. |

## Gates negativos obrigatórios

Mesmo request_id com conteúdo diferente; decision_id conflitante; versão de catálogo ausente; policy revogada entre planejamento e execução; journal corrompido; disco cheio; overload; atraso de descoberta; worker/planner indisponível; resultado final sem DONE e DONE sem output final; cancelamento anterior à materialização; tentativa/plan revision obsoleta; dois engines configurados no mesmo domínio.

Orçamento (V08/V09/V10): dois nós individualmente válidos cuja soma excede max_tokens do ingresso; planner que consome parte do saldo; forks reservando simultaneamente; retry após timeout sem consumo conhecido; replay da mesma reserva/outbox; preço desconhecido; worker redundante sem orçamento reservado; cliente DDS tentando contornar admissão. Então rejeitar antes de publicar chamada ou iniciar trabalho cobrado, sem saldo negativo, reserva duplicada ou ampliação silenciosa de limites. Política e pricing são fixados por revisão.

Auth DDS e confiança na identidade do anunciante precisam de cenário explícito com credenciais de teste antes de rede não confiável. MCP policy local não é evidência disso. Não armazenar segredos ou payloads sensíveis nos logs de aceite.
