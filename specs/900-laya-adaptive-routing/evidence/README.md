# Evidências M0

Artefatos desta execução documental. Não são resultados de Laya, campanha ou interop dos novos tópicos.

| Artefato | Conteúdo / limite |
|---|---|
| [before.json](before.json) | HEAD, branch, status após criação inicial do dossiê; hashes agregados do corpus protegido e hashes dos anexos. |
| [inventory.log](inventory.log) | Comandos/saídas de versões, dependentes NFCM, referências de roteamento, ignores, harnesses e fontes locais. |
| [workspace-test.json](workspace-test.json) / [log](workspace-test.log) | Primeira baseline, exit 101: falha no teste de permissão com capabilities de namespace root. Não é baseline verde. |
| [permission-recheck.json](permission-recheck.json) / [log](permission-recheck.log) | Mesmo teste exato com capabilities removidas; 1 passou, 29 filtrados. |
| [workspace-test-corrected.json](workspace-test-corrected.json) / [log](workspace-test-corrected.log) | Baseline completa repetida com isolamento corrigido; ver metadados e conclusão no REPORT. |
| [workspace-clippy.json](workspace-clippy.json) / [log](workspace-clippy.log) | Clippy workspace com -D warnings, exit 0. |
| [workspace-fmt.json](workspace-fmt.json) / [log](workspace-fmt.log) | Formatação somente check, exit 0; saída vazia é legítima. |
| [after.json](after.json) | Comparação final de HEAD/estado/hashes protegidos. |
| [document-validation.json](document-validation.json) | Verificação de arquivos, links, referências e escopo documental. |
| [review.md](review.md) | Revisão independente de consistência do SDD e tratamento dos achados. |

## Reprodutibilidade

Cada JSON de comando registra argv, cwd, environment overrides, wrapper de isolamento, timestamps UTC, exit code, duração e SHA-256 do log. Paths absolutos são metadados locais de evidência, não configuração de runtime a copiar para outra máquina. Não há dump de env nem credenciais.

`CYCLONEDDS_STATIC=1`; target exclusivo fora do CIFS; cargo offline e lockfile preservado; RUST_TEST_THREADS=1. A primeira execução reteve capabilities. A repetição configura loopback dentro do namespace e as remove antes de Cargo; não alterou o teste de permissão nem o runtime. Logs originais não são sobrescritos.

Manifesto protegido: SHA-256 de JSON compacto UTF-8 contendo lista ordenada de pares `(path relativo à raiz Rust, SHA-256 do conteúdo)`, com `ensure_ascii=False` e separadores `,`/`:`. Rastreados Rust excluem o dossiê novo. Conjuntos externos foram enumerados com `rg --files --hidden`, respeitando ignores e excluindo `.git`, `.venv`, `__pycache__`; symlinks não são seguidos. Contagens não devem ser somadas como arquivos únicos, porque há sobreposição.

Preservação não equivale a validação estatística dos resultados existentes. Sem pacote Laya ou pesos instalados; sem campanhas, rede de produção ou autenticação real no escopo desta entrega.
