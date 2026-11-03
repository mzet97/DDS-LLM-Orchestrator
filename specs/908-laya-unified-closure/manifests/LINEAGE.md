# T908 — Linhagem de checkpoints

## Árvore

```
convaiinnovations/laya (base SDK, 7b928d82…)
└── T900 specialized 35813054… (fase 900, rollback histórico)
    └── T904 Unified C bda679e7… (PROMOVIDO laya-production, PR #13 751cd96)  ← B0 / rollback T908
        ├── T905 N2 fa48606b… (OOD recalibração; caps F1=0 por 0 positivos — errata 5daa149)
        └── T906 C2 624de57e… (braço C 4/4 épocas; DOMINA B0 pareado no sealed T907)
            ├── T906 C1 ec40027a… (1ª execução braço C, INTERROMPIDA — não usar)
            └── T908 (braços a/b: init C2; braço c: init B0) ← ESTA CAMPANHA
```

## Papéis no T908

| ID | SHA-256 | Papel |
|---|---|---|
| B0 (T904) | `bda679e71295593904370eba5d9130609aea985f0b0d183d2527fcd398ca039a` | baseline pareado + init do braço c + rollback |
| N2 (T905) | `fa48606b…` | baseline pareado (sem treino T908) |
| C2 (T906) | `624de57e87bdae5d3da07f7e2a340a98005bc6ebbcbc99cf7f364345b50675db` | init dos braços a/b + dominância obrigatória vs finalista |
| T908 a/b/c | (a definir, fetch com SHA) | candidatos |

## Regras de preservação

- NENHUM checkpoint anterior é sobrescrito; T908 grava em `t908/models/arm_{a,b,c}/`
- Sealeds T902/T903/T905/T906/T907 JAMAIS entram em treino/seleção (8-gram bidirecional no `split_t908.py`)
- Ouro do suplemento/sealed T908: construção + filtro LLM; revisão humana segue PENDING
  (nunca confundir com concordância LLM×LLM)
- Promoção: só o autor decide; rollback = B0 (T904, vigente em laya-production)
