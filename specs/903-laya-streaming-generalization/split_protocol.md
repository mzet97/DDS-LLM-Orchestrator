# Protocolo de splits T903
Grupos: cenario (`group:train-scN` / `group:dev-scN` / `group:sealed-scN`) + banco de frases (train=C, sealed=D). Nenhum cenario ou grupo atravessa splits. Overlap exato verificado = 0 em todos os pares (script make_splits.py + augment).
Sealed: SHA 7578942a... (506 exemplos), lacrado em split_manifest.json; acesso restrito ate FREEZE + abertura unica documentada.
Versoes: seed 902/903 nos shuffles; manifestos com SHA por split.
