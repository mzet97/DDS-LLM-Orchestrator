# Dataset card T903 (revisao piloto aprovada em M2)
Origens: cenarios novos redigidos para T903 (PT/EN, 33 cenarios distintos) + replay limpo do pool antigo agrupado (regras textuais, sem modelo). Sem dados pessoais. Licenca: propria do projeto.
Proporcoes: train 1645 (STREAM 100), focal 2190 (STREAM 200), dev 150 (STREAM 42), cal 100, sealed 506 (STREAM 140, BALANCED 246).
Qualidade: checker de cues (0 STREAM sem entrega observavel; 0 FAILOVER sem continuidade); traps keyword->BALANCED; negacoes e escopo-code incluidos; overlap exato zero entre splits (ver split_manifest.json).
Limites: sintetico por construcao; sealed com frases-banco D e cenarios novos — distribuicao desenhada, nao natural.
