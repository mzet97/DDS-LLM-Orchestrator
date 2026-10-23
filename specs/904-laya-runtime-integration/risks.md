# T904 — Riscos

| Risco | Prob. | Impacto | Mitigação | Estado |
|---|---|---|---|---|
| Pesos T900 ausentes/divergentes | baixa | bloqueio total | hash EXATO verificado no M0 (`evidence/checkpoint_hashes_m0.txt`) | MITIGADO |
| Diretório incompleto (só pesos) | **certa** | load falha | montagem com complemento do BASE + manifest de hashes (M3) | TRATADO NO PLANO |
| Complemento do BASE divergir do fine-tune (ex.: config mudou no treino) | baixa | load/shape errado | treino `laya-train` não altera config/tokenizer (só pesos); smoke de carga M3 valida antes de qualquer uso; tamanho dos safetensors idêntico (±10 B de header) | TRATADO |
| Calibração inexistente para T900 | **certa** | decisões só shadow; M6 bloqueado | SHADOW_ONLY explícito; recalibração registrada com custo (CPU) — nunca reaproveitar calibração do BASE | ACEITO (honesto) |
| Fallback silencioso para modelo remoto | média | validade científica | 6 camadas de defesa (architecture.md); teste T-904-M2-05 | TRATADO |
| OOD: T900 treinado em sintético+SPROUT, textos do shadow podem desviar | média | decisões ruins (mas validadas) | validator Rust é o gate; shadow não materializa; achado v4 (choice_acc real 0.148 no BASE) já documentado | ACEITO/DOCUMENTADO |
| Custo de calibração/recalibração | média | horas de CPU ou custo GPU | estimativas no manifesto M4; só executa com autorização | REGISTRADO |
| Sobrescrever pesos/evidências T900–T903 | baixa | perda científica | T904 só LÊ os artefatos (symlinks); correções documentais datadas; nada é movido | TRATADO |
| SMB instável durante inferência (pesos no SMB) | média | worker lento/falho | load uma única vez no início; option de cópia local; SMB só na leitura inicial dos pesos | TRATADO |
| `torch` CPU-only local | certa | sem fast path; latência ~3 s/decisão | aceitável para shadow/E2E controlado; não alegar números de latência de GPU | ACEITO |
| HH 22 G livres no SMB | baixa | cópias falham | zero-cópia (symlinks); target dirs em /tmp e ~/.cache | TRATADO |
