# split_protocol — T904 Unified

## Regras

1. **Reused data (synthetic 2000, v4_train real, t901 270, hardnegs) ⇒ SEMPRE train.** Dev,
   calibration e sealed vêm EXCLUSIVAMENTE de famílias new-gen — elimina por construção o
   vazamento template/família que contaminou o holdout do T900 (85% T901).
2. Famílias new-gen = hash(domain, delivery_spec, lang) — lotes do mesmo cenário nunca se
   separam entre splits.
3. Divisão por hash de família + seed 904: sealed 16% das famílias, dev 10%, calibration 7%,
   resto train; greedy de balanceamento garante no sealed QI_STREAM_LIKE ≥ 40 e
   QI_FAILOVER ≥ 25 movendo famílias inteiras.
4. `split_manifest.json` congelado com SHAs e contagens por classe ANTES do treino; sealed
   gravado FORA da árvore de trabalho com chmod 000 e hash registrado; treino/tuning não
   recebem os caminhos.
5. Sealed anônimo para upload (sem expected/_meta); labels ficam locais e sobem apenas na
   abertura única (M7).
6. Auditoria: 8-gramas de TODA partição contra `v4_test.jsonl` (local) — hits ⇒ família
   excluída; golden450/sealed T902/T903 não estão no disco local ⇒ proteção por construção
   (gerador novo, famílias novas, proibição registrada) — limitação documentada.

## Tamanhos-alvo (precisão sobre IC, não número mágico)

Sealed ~450–550 casos com as 5 classes QoS representadas, casos com e sem literal "stream",
urgência expressa/ambígua, todas as capabilities; dev ~300; calibration ~200.
