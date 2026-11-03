#!/usr/bin/env python3
"""T904-M1 — hard negatives determinísticos (sem LLM): termos temáticos de
"stream" e advérbios de velocidade SEM pedido de entrega parcial => QI_BALANCED;
pares mínimos de entrega; deadlines que mapeiam para urgency, não para qos.
Tier: template-hardneg. Saída: JSONL {id, lang, text, family, stratum}.
"""
import hashlib
import json
import sys

SUBJECTS = [
    ("pt", "Configure um consumer de {tech} para processar eventos de pedidos e gerar um relatório consolidado ao final."),
    ("pt", "Preciso de um serviço que leia do {tech}, cruze com o cadastro de clientes e entregue o resultado completo no fim."),
    ("en", "Set up a {tech} consumer to process order events and produce a consolidated report at the end."),
    ("en", "I need a service that reads from {tech}, joins it with the customer registry, and delivers the full result when done."),
]
TECHS = ["Kafka Streams", "AWS Kinesis", "um stdout pipe", "um stream de logs do syslog", "um fluxo SSE interno",
         "um pipeline de streaming de vídeo", "um topic Redis Streams"]
ADVERBS = [
    ("pt", "Rode tudo {adv} e me entregue o relatório final consolidado.", "rapidamente"),
    ("pt", "Execute {adv} o backfill completo do histórico; quero apenas o resultado final.", "em paralelo"),
    ("pt", "Preciso disso {adv}, mas não me mande nada parcial — só o produto final.", "o mais rápido possível"),
    ("en", "Run it {adv} and give me only the final consolidated report.", "as fast as possible"),
    ("en", "Process the backlog {adv}; do not send partial outputs.", "in parallel"),
    ("en", "I need this {adv} but the deliverable is a single final artifact.", "asynchronously"),
]
PAIRS = [
    ("pt", "STREAM", "Analise os {n} vídeos e me envie o resumo de cada um assim que terminar cada análise."),
    ("pt", "BALANCED", "Analise os {n} vídeos e me entregue o resumo completo de todos ao final, em um único documento."),
    ("en", "STREAM", "Review the {n} pull requests and send me each verdict as soon as you finish reviewing it."),
    ("en", "BALANCED", "Review the {n} pull requests and deliver one consolidated verdict report at the end."),
    ("pt", "STREAM", "Traduza os documentos e vá me devolvendo as versões parciais conforme ficar prontas."),
    ("pt", "BALANCED", "Traduza os documentos e me devolva tudo pronto de uma vez, sem versões intermediárias."),
    ("en", "STREAM", "Audit the services and stream me the findings incrementally while the audit runs."),
    ("en", "BALANCED", "Audit the services and send a single final findings report when the audit completes."),
]
DEADLINE_PAIRS = [
    ("pt", "Preciso do pipeline {d}. Qualidade em primeiro lugar, entrega única no final.", "para ontem"),
    ("pt", "Preciso do pipeline {d}. Qualidade em primeiro lugar, entrega única no final.", "até o fim do mês, sem pressa"),
    ("en", "The migration is {d}. Reliability first; single final deliverable.", "due in 2 hours"),
    ("en", "The migration is {d}. Reliability first; single final deliverable.", "due sometime next month"),
]


def fam(*parts) -> str:
    return "hn-" + hashlib.sha1("|".join(parts).encode()).hexdigest()[:10]


def main() -> int:
    out = sys.argv[1] if len(sys.argv) > 1 else "hardnegs.jsonl"
    rows = []
    for i, (lang, tpl) in enumerate(SUBJECTS):
        for tech in TECHS:
            rows.append({"id": f"hn-subj-{i}-{abs(hash(tech)) % 9999}", "lang": lang,
                         "text": tpl.format(tech=tech), "stratum": "subject-stream-term",
                         "family": fam("subj", lang, tpl, tech)})
    for i, (lang, tpl, adv) in enumerate(ADVERBS):
        rows.append({"id": f"hn-adv-{i}", "lang": lang, "text": tpl.format(adv=adv),
                     "stratum": "adverb-no-partial", "family": fam("adv", lang, tpl)})
    n_counts = ["3", "12", "40", "150"]
    for i, (lang, label, tpl) in enumerate(PAIRS):
        for j, n in enumerate(n_counts):
            rows.append({"id": f"hn-pair-{i}-{j}", "lang": lang, "text": tpl.format(n=n),
                         "stratum": f"pair-{label.lower()}", "family": fam("pair", lang, tpl)})
    for i, (lang, tpl, d) in enumerate(DEADLINE_PAIRS):
        rows.append({"id": f"hn-dl-{i}", "lang": lang, "text": tpl.format(d=d),
                     "stratum": "deadline-not-qos", "family": fam("dl", lang, tpl, d)})
    with open(out, "w") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"{len(rows)} hard negatives -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
