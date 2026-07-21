# Tasks 400 — Baselines + consolidação

- [x] **T-501 · trait QosDecider + estático** (REQ-501, REQ-504)
  **Status:** ✅ — trait comum (outra sessão) + `impl QosDecider for Nfcm` adicionado;
  5 braços atrás da mesma trait.
- [x] **T-502 · Seletor linear (Zadeh)** (REQ-502)
  **Status:** ✅ — reescrito como **porte fiel** (α-cuts + Princípio de Extensão +
  pesos canônicos); `tests/zadeh_parity.rs`: seleções e centroids **exatos** vs Python
  (1e-9, 6 cenários).
- [x] **T-503 · FCM + DHL** (REQ-503)
  **Status:** ✅ — reescrito como **porte fiel** (clamp de entradas, detecção de atrator,
  DHL Kosko sobre estado completo); `tests/fcm_parity.rs`: paridade vs Python (1e-4,
  7 iterações fixed_point); **divergência real vs linear em 'lote barato'**;
  DHL converge para a correlação observada.
- [x] **T-504 · `--qos-manager` com os 5 modos** (REQ-504)
  **Status:** ✅ — orchestrator `--qos-manager {static,zadeh,fcm,fcm-dhl,nfcm}`;
  `tests/qos_manager.rs`: 5 modos rodam e decidem corretamente.
- [x] **T-505 · Harness de 5 braços** (REQ-505)
  **Status:** ✅ — `qos-nfcm/examples/five_arms.rs`: tabela 6 cenários × 5 braços
  (perfil, confiança, ns/decide local) + divergências.
- [x] **T-506 · Arquivar Python + E2E + REPORT** (REQ-506, gate)
  **Status:** ✅ — 3 pacotes + 10 testes Python movidos para
  `archive/python_qos_baselines/` (README de aposentadoria); E2E Rust-only revalidado;
  REPORT.md escrito.

## Gate de saída (Fase 4 / migração)
5 braços comparáveis em Rust ✓ · Python equivalente arquivado ✓ · E2E verde ✓ · REPORT final ✓
