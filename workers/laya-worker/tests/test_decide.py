"""Unit tests do decide (T-900-M2-02): correlação, versão, erros tipados."""

import sys
import unittest
from dataclasses import dataclass
from pathlib import Path

WORKER = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKER / "src"))

from laya_worker import adapter, decide
from laya_worker.questions import QUESTIONS_REVISION


@dataclass
class FakeIntent:
    request_id: str = "req-1"
    workflow_id: str = "wf-1"
    schema_version: int = 1
    intent_revision: int = 1
    prompt: str = "audit this rust service and create tests"
    created_at_ns: int = 1_760_000_000_000_000_000
    deadline_ns: int = 1_760_000_600_000_000_000
    questions_revision: int = QUESTIONS_REVISION
    policy_revision: int = 7
    capability_catalog_revision: int = 1
    checkpoint_revision: str = "a" * 64


class OkDecisionTest(unittest.TestCase):
    def test_ok_decision_fields(self):
        d = decide.decide(FakeIntent(), adapter.FakeLayaAdapter(), now_ns=1_760_000_005_000_000_001)
        self.assertEqual(d["decision_status"], 0)
        self.assertEqual(d["reason_code"], 0)
        self.assertEqual(d["request_id"], "req-1")
        self.assertEqual(d["decision_id"], "dec-req-1")
        self.assertEqual(d["workflow_id"], "wf-1")
        self.assertEqual(d["schema_version"], 1)
        self.assertIn(d["qos_profile_label"],
                      ("QI_CRITICAL", "QI_BALANCED", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"))
        # correlação de revisões: copiadas da intenção
        self.assertEqual(d["questions_revision"], QUESTIONS_REVISION)
        self.assertEqual(d["policy_revision"], 7)
        # distribuições completas (5 perfis)
        self.assertEqual(len(d["qos_probabilities"]), 5)
        self.assertAlmostEqual(sum(d["qos_probabilities"].values()), 1.0, places=5)
        self.assertEqual(len(d["capability_probabilities"]), 8)  # catálogo completo
        self.assertEqual(len(d["priority_probabilities"]), 3)
        # valid_until <= deadline da intenção
        self.assertLessEqual(d["valid_until_ns"], FakeIntent.deadline_ns)
        # shadow: calibração vazia
        self.assertEqual(d["calibration_revision"], "")
        # payload_hash é sha256 hex de 64
        self.assertEqual(len(d["payload_hash"]), 64)
        int(d["payload_hash"], 16)

    def test_ttl_never_exceeds_deadline(self):
        d = decide.decide(FakeIntent(), adapter.FakeLayaAdapter(), ttl_s=9999.0,
                          now_ns=1_760_000_005_000_000_001)
        self.assertEqual(d["valid_until_ns"], FakeIntent.deadline_ns)


class ErrorMappingTest(unittest.TestCase):
    def _err(self, **kw) -> dict:
        intent = FakeIntent(**kw)
        return decide.decide(intent, adapter.FakeLayaAdapter(),
                             now_ns=1_760_000_005_000_000_001)

    def test_questions_revision_mismatch_is_schema_mismatch(self):
        d = self._err(questions_revision=99)
        self.assertEqual(d["decision_status"], 1)
        self.assertEqual(d["reason_code"], 4)  # RC_SCHEMA_MISMATCH
        self.assertEqual(d["qos_probabilities"], [])
        self.assertNotEqual(d["payload_hash"], "")

    def test_unavailable_adapter_maps_to_model_unavailable(self):
        class Dead:
            def decide(self, prompt, deadline_ns):
                raise adapter.LayaUnavailableError("sdk ausente")

        d = decide.decide(FakeIntent(), Dead(), now_ns=1)
        self.assertEqual(d["reason_code"], 2)  # RC_MODEL_UNAVAILABLE

    def test_timeout_maps_to_timeout(self):
        class Slow:
            def decide(self, prompt, deadline_ns):
                raise adapter.LayaTimeoutError()

        d = decide.decide(FakeIntent(), Slow(), now_ns=1)
        self.assertEqual(d["reason_code"], 3)  # RC_TIMEOUT

    def test_inconsistent_answer_maps_to_output_invalid(self):
        class Liar:
            def __init__(self):
                self.n = 0

            def decide(self, prompt, deadline_ns):
                a = adapter.FakeLayaAdapter().decide(prompt, deadline_ns)
                if self.n == 0:
                    self.n += 1
                    a.qos_probabilities = {"QI_BALANCED": 0.3}  # soma != 1
                return a

        d = decide.decide(FakeIntent(), Liar(), now_ns=1)
        self.assertEqual(d["reason_code"], 12)  # RC_OUTPUT_INVALID

    def test_error_decisions_still_correlate(self):
        d = self._err(questions_revision=99)
        self.assertEqual(d["request_id"], "req-1")
        self.assertEqual(d["decision_id"], "dec-req-1")


class DeterminismTest(unittest.TestCase):
    def test_same_intent_same_hash(self):
        i = FakeIntent()
        laya = adapter.FakeLayaAdapter()
        d1 = decide.decide(i, laya, now_ns=42)
        d2 = decide.decide(i, laya, now_ns=42)
        self.assertEqual(d1["payload_hash"], d2["payload_hash"])

    def test_different_prompt_different_hash(self):
        d1 = decide.decide(FakeIntent(prompt="audit rust"), adapter.FakeLayaAdapter(), now_ns=42)
        d2 = decide.decide(FakeIntent(prompt="translate doc"), adapter.FakeLayaAdapter(), now_ns=42)
        self.assertNotEqual(d1["payload_hash"], d2["payload_hash"])


if __name__ == "__main__":
    unittest.main()
