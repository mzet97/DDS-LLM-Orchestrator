"""Adapter Laya mockável (T-900-M2-01; ADR-001/D9).

`LayaAdapter` é o ÚNICO ponto de contato com o modelo. O `FakeLayaAdapter`
usa o baseline de regras (determinístico, offline, sem pesos) — é o adapter
dos testes unitários e do shadow de ensaio. O `RealLayaAdapter` importa o SDK
LAZY (sem rede/pesos em unit) e hoje levanta `LayaUnavailableError` até o pin
de versão/checkpoint/hardware (R-LAYA BLOCKED — decisão do usuário).
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol

from . import rules_baseline
from .questions import CAPABILITY_CATALOG, QOS_PROFILES, QUESTIONS_REVISION, QUESTIONS_V1


class LayaUnavailableError(RuntimeError):
    """SDK Laya ausente/não pinado — inferência real BLOCKED (R-LAYA)."""


class CheckpointMissingError(LayaUnavailableError):
    """checkpoint_path inexistente (T904/REQ-904-002) — sem fallback."""


class CheckpointIncompleteError(LayaUnavailableError):
    """Diretório do checkpoint sem arquivos exigidos pelo SDK (T904/REQ-904-002)."""


class CheckpointHashMismatchError(LayaUnavailableError):
    """sha256 do model.safetensors diverge do pin esperado (T904/REQ-904-002)."""


class LayaTimeoutError(TimeoutError):
    """Resposta do modelo excedeu o prazo."""


@dataclass
class LayaAnswer:
    """Resposta tipada do Laya para o conjunto QUESTIONS_V1.

    Probabilidades em [0,1]; distribuições somam 1 (tol. 1e-6);
    `complexity_expected`/`urgency_expected` são valores ESPERADOS na escala
    0..3 / 0..2 (score é esperança, NÃO categoria arredondada — contracts.md).
    """

    qos_profile: str
    qos_probabilities: dict[str, float]
    needs_planning_probability: float
    capability_probabilities: dict[str, float]
    capability_answer_confidences: dict[str, float]
    complexity_expected: float
    urgency_expected: float
    priority_probabilities: list[float] = field(default_factory=lambda: [1 / 3, 1 / 3, 1 / 3])
    confidence: float = 0.5
    answer_confidence: float = 0.5
    model_id: str = "rules-baseline-v1"
    checkpoint_revision: str = ""
    # A3: pin da calibração carregada (vazio = sem calibração; shadow)
    calibration_revision: str = ""


class LayaAdapter(Protocol):
    def decide(self, prompt: str, deadline_ns: int) -> LayaAnswer:
        ...


class FakeLayaAdapter:
    """Adapter offline/determinístico sobre o baseline de regras (M2-03).

    Sem rede, sem pesos, sem download — usado por unit tests e shadow de
    ensaio. Confianças fixas e HONESTAMENTE baixas (não calibrado).
    """

    def __init__(self, confidence: float = 0.55) -> None:
        self.confidence = confidence
        self.questions = QUESTIONS_V1

    def decide(self, prompt: str, deadline_ns: int) -> LayaAnswer:
        pred = rules_baseline.predict(prompt)
        return LayaAnswer(
            qos_profile=pred.qos_profile,
            qos_probabilities=pred.qos_probabilities,
            needs_planning_probability=pred.needs_planning_probability,
            capability_probabilities=pred.capability_probabilities,
            capability_answer_confidences={
                k: 0.5 for k in pred.capability_probabilities
            },
            complexity_expected=pred.complexity_expected,
            urgency_expected=pred.urgency_expected,
            priority_probabilities=pred.priority_probabilities,
            confidence=self.confidence,
            answer_confidence=self.confidence,
            model_id=f"fake:{rules_baseline.MODEL_ID}",
            checkpoint_revision="",
        )


# Pin do checkpoint (R-LAYA DESBLOQUEADO 2026-10-17, autorização do usuário):
CHECKPOINT_SNAPSHOT = "7b928d828b7b0e022f929d9bd2e44165aa270148"
CHECKPOINT_MODEL_SHA256 = "891102d372688fc2a094dac56a384bc537b87c63f21f9f3dac0be2b7cbc8d86c"
CHECKPOINT_CONFIG_SHA256 = "ae287b56bbcf5f8c4f4541ae9dfd00c914c4c48b940b8398c3058af37ba92bbd"
PINNED_SDK = "laya==0.4.10 (Apache-2.0)"

# T904/REQ-904-002: arquivos que o laya==0.4.10 exige num diretório de checkpoint
# (agent.py: rl_agent_config.json + model.safetensors + tokenizer/* + encoder/*).
# Pesos sozinhos NÃO são carregáveis — ver workers/laya-worker/tools/assemble_checkpoint.py.
CHECKPOINT_REQUIRED_FILES = (
    "model.safetensors",
    "rl_agent_config.json",
    "encoder/config.json",
    "tokenizer/tokenizer_config.json",
    "tokenizer/tokenizer.json",
)


def _sha256_file(path: str) -> str:
    """SHA-256 em stream (arquivos de ~842 MB não cabem em slurp)."""
    import hashlib as _h

    h = _h.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _laya_load(path: str, expected_sha256: dict[str, str] | None = None):
    """Ponto ÚNICO de carga do SDK (injetável nos testes; import LAZY).

    `expected_sha256` usa a verificação NATIVA do SDK ({caminho relativo: hexdigest}).
    """
    import laya

    return laya.load(path, expected_sha256=expected_sha256)


def _laya_questions() -> dict:
    """QUESTIONS_V1 no formato do SDK: choice + bateria noul por capability + scores."""
    q = {
        "qos_profile": QUESTIONS_V1["qos_profile"],
        "needs_planning": QUESTIONS_V1["needs_planning"],
        "complexity": QUESTIONS_V1["complexity"],
        "urgency": QUESTIONS_V1["urgency"],
    }
    for cap in CAPABILITY_CATALOG:
        q[f"cap__{cap}"] = {
            "type": "noul",
            "instructions": f"Does this request require the capability '{cap}'?",
        }
    return q


class RealLayaAdapter:
    """Adapter do SDK Laya REAL (pinado; desbloqueado por autorização).

    Import LAZY de `laya` (nunca no caminho de unit tests). Dois modos:

    - LOCAL (T904): `checkpoint_path` + `checkpoint_sha256` obrigatórios. O
      sha256 do `model.safetensors` é calculado ANTES do load (erros tipados
      `CheckpointMissingError`/`CheckpointIncompleteError`/
      `CheckpointHashMismatchError`); o load é offline e SEM fallback para o
      ID remoto. Identidade: `model_id` = `local:<dir>` (ou explícito),
      `checkpoint_revision` = hash verificado.
    - LEGADO (sem `checkpoint_path`): ID remoto do BASE pinado — mantido para
      reprodução dos experimentos de calibração v1–v4.

    Timeout: inferência roda em thread com prazo derivado do deadline da
    intenção (`LayaTimeoutError` tipado).

    Nota de calibração (honestidade/D10): as calibrações existentes (v1–v4)
    pertencem ao BASE; sem calibração vinculada ao checkpoint carregado,
    `calibration_revision` fica vazia e o validator Rust mantém a decisão em
    shadow.
    """

    EXPECTED_SDK = PINNED_SDK
    MODEL_ID = "convaiinnovations/laya"

    def __init__(self, checkpoint_sha256: str | None = None,
                 calibration_path: str | None = None,
                 checkpoint_path: str | None = None,
                 model_id: str | None = None) -> None:
        self.checkpoint_sha256 = checkpoint_sha256 or CHECKPOINT_MODEL_SHA256
        if len(self.checkpoint_sha256) != 64:
            raise LayaUnavailableError(
                "checkpoint_sha256 deve ser o sha256 hex (64) do model.safetensors"
            )
        self.questions = QUESTIONS_V1
        self._agent = None
        # A3: calibração opcional — se fornecida, o pin vira o
        # calibration_revision (validador Rust só aceita pin casando).
        self.calibration_path = calibration_path
        self.calibration_pin = ""
        if calibration_path:
            import hashlib as _h
            from pathlib import Path as _P
            blob = _P(calibration_path).read_bytes()
            self.calibration_pin = _h.sha256(blob).hexdigest()

        # T904/REQ-904-001: modo local EXPLÍCITO. Sem checkpoint_path, o modo
        # legado (ID remoto do BASE, reprodução de experimentos) é preservado.
        self.checkpoint_dir = ""
        self._expected_digests: dict[str, str] | None = None
        if checkpoint_path is None:
            self.model_id = model_id or self.MODEL_ID
            return

        path = Path(checkpoint_path)
        if path.is_file():  # aceita <dir>/model.safetensors ou o diretório
            path = path.parent
        if not path.is_dir():
            raise CheckpointMissingError(f"checkpoint_path inexistente: {checkpoint_path}")
        missing = [rel for rel in CHECKPOINT_REQUIRED_FILES if not (path / rel).is_file()]
        if missing:
            raise CheckpointIncompleteError(
                f"checkpoint incompleto em {path}; faltam (monte com "
                f"tools/assemble_checkpoint.py): {', '.join(missing)}"
            )
        got = _sha256_file(str(path / "model.safetensors"))
        if got != self.checkpoint_sha256:
            raise CheckpointHashMismatchError(
                f"checkpoint divergente do pin: {got} != {self.checkpoint_sha256}"
            )
        self.checkpoint_dir = str(path.resolve())
        self._expected_digests = {"model.safetensors": got}
        self.checkpoint_sha256 = got
        self.model_id = model_id or f"local:{path.name}"

    def _load(self):
        if self._agent is not None:
            return self._agent
        if self.checkpoint_dir:
            return self._load_local()
        try:
            import laya
        except ImportError as e:
            raise LayaUnavailableError(
                f"SDK Laya não instalado ({e}); esperado: {self.EXPECTED_SDK}"
            ) from e
        import glob as _glob

        snaps = _glob.glob(
            f"{_home_hf()}/hub/models--convaiinnovations--laya/snapshots/*/model.safetensors"
        )
        if snaps:
            got = _sha256_file(snaps[0])
            if got != self.checkpoint_sha256:
                raise CheckpointHashMismatchError(
                    f"checkpoint divergente do pin: {got} != {self.checkpoint_sha256}"
                )
        self._agent = _laya_load(self.MODEL_ID)
        if self.calibration_path:
            self._agent.load_calibration(self.calibration_path)
        return self._agent

    def _load_local(self):
        """T904/REQ-904-002+003: carrega SÓ o diretório local verificado.

        Hash conferido no __init__ ANTES daqui; offline forçado durante o load
        (sem registry/alias/rede); o path com separador faz o SDK tratar como
        checkpoint local. Nenhum fallback para `convaiinnovations/laya`.
        """
        import os as _os

        saved = {k: _os.environ.get(k) for k in ("HF_HUB_OFFLINE", "TRANSFORMERS_OFFLINE")}
        for k in saved:
            _os.environ[k] = "1"
        try:
            self._agent = _laya_load(self.checkpoint_dir,
                                     expected_sha256=self._expected_digests)
        finally:
            for k, prev in saved.items():
                if prev is None:
                    _os.environ.pop(k, None)
                else:
                    _os.environ[k] = prev
        if self.calibration_path:
            self._agent.load_calibration(self.calibration_path)
        return self._agent

    def decide(self, prompt: str, deadline_ns: int) -> LayaAnswer:
        import concurrent.futures as _cf
        import time as _time

        agent = self._load()
        questions = _laya_questions()
        budget_s = None
        if deadline_ns > 0:
            now = _time.time_ns()
            if deadline_ns <= now:
                raise LayaTimeoutError("deadline da intenção já vencido")
            budget_s = max(0.05, (deadline_ns - now) / 1e9)

        with _cf.ThreadPoolExecutor(max_workers=1) as pool:
            fut = pool.submit(agent.system_one, {"text": prompt}, questions)
            try:
                result = fut.result(timeout=budget_s)
            except _cf.TimeoutError as e:
                raise LayaTimeoutError(
                    f"inferência excedeu {budget_s:.1f}s do deadline"
                ) from e

        answers = result.get("answers", {})

        def _choice(name):
            a = answers.get(name, {})
            probs = {k: float(v) for k, v in (a.get("probabilities") or {}).items()}
            if not probs:
                probs = {a.get("choice", "QI_BALANCED"): 1.0}
            total = sum(probs.values()) or 1.0
            probs = {k: v / total for k, v in probs.items()}
            return a.get("choice", "QI_BALANCED"), probs, a

        qos_profile, qos_probs, qos_ans = _choice("qos_profile")
        # domínio completo 0..1 (ordem do contrato):
        full = {p: qos_probs.get(p, 0.0) for p in QOS_PROFILES}
        total = sum(full.values())
        if total > 0:
            full = {k: v / total for k, v in full.items()}

        needs = float(answers.get("needs_planning", {}).get("noul", 0.0))

        cap_probs, cap_conf = {}, {}
        for cap in CAPABILITY_CATALOG:
            a = answers.get(f"cap__{cap}", {})
            p = float(a.get("noul", 0.0))
            if 0.0 <= p <= 1.0:
                cap_probs[cap] = p
                cap_conf[cap] = float(a.get("answer_confidence", p))

        def _score_expected(name):
            a = answers.get(name, {})
            return float(a.get("score", 1.0)), {
                k: float(v) for k, v in (a.get("probabilities") or {}).items()
            }

        complexity, _ = _score_expected("complexity")
        urgency, urgency_probs = _score_expected("urgency")
        prio3 = [
            urgency_probs.get("0", urgency_probs.get("low", 1 / 3)),
            urgency_probs.get("1", urgency_probs.get("normal", 1 / 3)),
            urgency_probs.get("2", urgency_probs.get("high", 1 / 3)),
        ]
        s = sum(prio3) or 1.0
        prio3 = [p / s for p in prio3]

        answer = LayaAnswer(
            qos_profile=qos_profile,
            qos_probabilities=full,
            needs_planning_probability=min(1.0, max(0.0, needs)),
            capability_probabilities=cap_probs,
            capability_answer_confidences=cap_conf,
            complexity_expected=min(3.0, max(0.0, complexity)),
            urgency_expected=min(2.0, max(0.0, urgency)),
            priority_probabilities=prio3,
            confidence=float(qos_ans.get("confidence", 0.0)),
            answer_confidence=float(qos_ans.get("answer_confidence", 0.0)),
            model_id=self.model_id,
            checkpoint_revision=self.checkpoint_sha256,
            calibration_revision=self.calibration_pin,
        )
        if not answer_is_self_consistent(answer):
            raise ValueError("resposta do Laya fora do domínio (verifique checkpoint)")
        return answer


def _home_hf() -> str:
    import os

    return os.environ.get(
        "HF_HOME", os.path.expanduser("~/.cache/huggingface")
    )


def answer_is_self_consistent(a: LayaAnswer) -> bool:
    """Validação LOCAL do adapter (D10: dado não confiável até validação)."""
    def ok(p: float) -> bool:
        return math.isfinite(p) and 0.0 <= p <= 1.0

    if a.qos_profile not in QOS_PROFILES:
        return False
    if a.qos_profile not in a.qos_probabilities:
        return False
    if set(a.qos_probabilities) - set(QOS_PROFILES):
        return False
    if set(a.capability_probabilities) - set(CAPABILITY_CATALOG):
        return False
    if any(not ok(p) for p in a.qos_probabilities.values()):
        return False
    if abs(sum(a.qos_probabilities.values()) - 1.0) > 1e-6:
        return False
    if not ok(a.needs_planning_probability):
        return False
    if any(not ok(p) for p in a.capability_probabilities.values()):
        return False
    if any(not ok(p) for p in a.capability_answer_confidences.values()):
        return False
    if not (0.0 <= a.complexity_expected <= 3.0) or not (0.0 <= a.urgency_expected <= 2.0):
        return False
    if len(a.priority_probabilities) != 3 or any(not ok(p) for p in a.priority_probabilities):
        return False
    if abs(sum(a.priority_probabilities) - 1.0) > 1e-6:
        return False
    if not ok(a.confidence) or not ok(a.answer_confidence):
        return False
    return True
