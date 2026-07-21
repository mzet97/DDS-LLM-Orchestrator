# Spec 400 — Baselines + consolidação

**Fase:** 4 · **Crate:** `qos-nfcm` (estende) + limpeza · **Depende de:** 300.

## Por quê
O artigo compara 5 braços: **estático · Zadeh (linear) · FCM (pesos fixos) · FCM+DHL ·
NFCM**. O NFCM já está pronto; faltam os baselines em Rust e o desligamento do Python
equivalente. Fecha a migração.

## O quê (requisitos)
- **REQ-501 — Seletor estático.** Perfil fixo (controle). *Aceite:* sempre devolve o mesmo.
- **REQ-502 — Seletor linear (Zadeh).** Score ponderado por perfil (paridade com
  `fuzzy_qos_manager`). *Aceite:* discrimina cenários como o Python.
- **REQ-503 — FCM + DHL.** Motor FCM de Kosko + aprendizado Hebbiano (paridade com
  `fcm_qos_manager`). *Aceite:* reproduz a discriminação/atrator do Python.
- **REQ-504 — Seleção de decisor.** `--qos-manager {static,zadeh,fcm,fcm-dhl,nfcm}` no
  orchestrator. *Aceite:* cada modo roda no loop de controle.
- **REQ-505 — Harness de 5 braços.** Comparar os 5 decisores sobre o mesmo trace/carga,
  reportando as métricas do artigo (latência, prazos, erros, trocas de perfil, convergência).
  *Aceite:* relatório com os 5 braços (números do que for medível localmente; o resto é cluster).
- **REQ-506 — Desligar o Python equivalente.** Arquivar `fuzzy_qos_manager`/`fcm_qos_manager`
  Python (não apagar) quando o Rust tiver paridade. *Aceite:* nota de arquivamento + E2E verde.

## Fora de escopo
- Executar as campanhas reais no cluster (é do `opencode_deve_fazer.md §7`). Aqui é o
  **mecanismo** de comparação; os números de desempenho de QoS vêm do cluster.
