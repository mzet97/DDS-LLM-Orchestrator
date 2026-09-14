# Registro de promoção — tese-rust 54a7c34 (F2, 2026-09-14)

**Finalidade: APROVADO PARA ENSAIO F3 ISOLADO (host descartável 192.168.1.62
exclusivamente); NÃO aprovado para substituir serviços operacionais
(studio-noded operacional, agentes, orquestradores, inferência).**

| Campo | Valor |
|---|---|
| Revisão-fonte (código compilado) | `54a7c349973b012d05a8ad277133fcb21cba9ac1` |
| Revisão do workflow | `54a7c349973b012d05a8ad277133fcb21cba9ac1` (ci.yml da própria revisão; jobs no mesmo SHA) |
| Execução no Gitea | run nº **8** (tasks 108–111; event workflow_dispatch; todos SUCCESS) |
| Artefato de origem | **ID 5** no serviço de artefatos do Gitea (`gitea_admin/DDS-LLM-Orchestrator`), vínculo sha `54a7c349…` confirmado pela API antes do download |
| Nome exato do arquivo | `tese-rust-x86_64-54a7c349973b012d05a8ad277133fcb21cba9ac1.tar.gz` |
| SHA-256 do pacote (.tar.gz) | `b4f892c7acbb37c4cb968d35771a65253e07d46bceee1ca66281c7a38cc4a7fa` |
| Referência OCI da aplicação | `harbor.home.arpa/tese/tese-rust@sha256:ca4836d7831fb07fb6fb869439f62d52ca172c763169226c2f2e068892382a76` (tag candidata `54a7c34-x86_64`; artifact-type `application/vnd.tese.bundle`) |
| Digest da IMAGEM do runner (distinto) | `tese-runner@sha256:cee130476b844294dc670b258bd3f6adbdbcb84d3d5762929bb52dc0a558917d` (imagem de CI; NÃO confundir com o digest do pacote acima) |

## Verificações executadas antes da promoção
- Vínculos artefato↔run↔revisão confirmados pela API (artefato ID 5, head_sha
  `54a7c349…`, run nº 8 — não selecionado "o mais recente").
- Download do serviço de artefatos em diretório novo; zip contém EXATAMENTE
  o arquivo nominal acima.
- Pacote verificado: `manifest.json` (commit `54a7c349…`, arch `x86_64`,
  toolchain `rustc 1.95.0`), 6 binários obrigatórios (studio-noded, agent,
  orchestrator, submit-one, wf-run, det-responder) + `lib/libddsc.so`,
  `lib/libddsc.so.11`, `lib/libddsc.so.11.0.0`; `sha256sum -c SHA256SUMS`
  integral (9/9 OK).
- Publicado o MESMO .tar.gz (sem recompilar/reempacotar) via `oras push`
  (HTTPS harbor.home.arpa + CA local; credencial robot de publicação
  `robot$tese+tese-ci`, segredo apenas em arquivo 0600 da operadora).
- Pull por digest em outro diretório: SHA-256 do arquivo recuperado
  **idêntico** ao original (`b4f892c7…a7fa`).
