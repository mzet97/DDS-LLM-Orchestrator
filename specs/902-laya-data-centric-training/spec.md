# Especificacao T902
- REQ-902-01 Integridade: T900 e datasets originais preservados por hash.
- REQ-902-02 Rotulos: politica rev.1 anterior ao treino; ambiguos fora da supervisao.
- REQ-902-03 Splits: grupos (familia/template/par/fonte) inteiros num split; sealed T902 aberto 1x apos freeze.
- REQ-902-04 Treino: Colab CLI T4, validate+dry-run, logs separados, persistencia incremental com SHA.
- REQ-902-05 Avaliacao: por classe/pergunta, MAE ordinal, ECE/Brier so choice/noul validos, bootstrap pareado agrupado.
- REQ-902-06 Promocao: somente com ganho IC + nao-regressao; senao KEEP_T900/INCONCLUSIVE (decision.json).
- REQ-902-07 Integracao: validator Rust + shadow real tasks=0; sem materializar; sem mudar contratos.
