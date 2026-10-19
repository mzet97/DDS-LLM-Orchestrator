"""Unit tests do adapter (T-900-M2-01): sem pesos, sem rede, sem download."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from laya_worker import adapter, rules_baseline


class FakeAdapterTest(unittest.TestCase):
    def test_fake_is_deterministic_and_offline(self):
        a = adapter.FakeLayaAdapter()
        r1 = a.decide("audit this rust service and create tests", 999)
        r2 = a.decide("audit this rust service and create tests", 999)
        self.assertEqual(r1.qos_profile, r2.qos_profile)
        self.assertEqual(r1.qos_probabilities, r2.qos_probabilities)
        self.assertTrue(r1.model_id.startswith("fake:"))
        self.assertEqual(r1.checkpoint_revision, "")  # regras: sem checkpoint

    def test_fake_answer_is_self_consistent(self):
        a = adapter.FakeLayaAdapter()
        for prompt in ("resuma este artigo", "streaming ao vivo",
                       "urgent production incident", "translate this"):
            with self.subTest(prompt=prompt):
                self.assertTrue(adapter.answer_is_self_consistent(a.decide(prompt, 1)))


class RealAdapterGateTest(unittest.TestCase):
    def test_real_requires_checkpoint_pin(self):
        # A3: checkpoint tem DEFAULT PINADO (desbloqueio 2026-10-17); o gate
        # agora é o PIN DIVERGENTE, não a ausência.
        with self.assertRaises(adapter.LayaUnavailableError):
            adapter.RealLayaAdapter(checkpoint_sha256="curto")

    def test_real_accepts_pinned_default_and_calib_pin(self):
        real = adapter.RealLayaAdapter()  # default pinado
        self.assertEqual(len(real.checkpoint_sha256), 64)
        self.assertEqual(real.calibration_pin, "")  # sem calibração
        import hashlib, tempfile, os
        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
            f.write(b"{}")
            path = f.name
        try:
            real2 = adapter.RealLayaAdapter(calibration_path=path)
            self.assertEqual(len(real2.calibration_pin), 64)
        finally:
            os.unlink(path)

    def test_real_decide_blocked_without_sdk(self):
        real = adapter.RealLayaAdapter(checkpoint_sha256="a" * 64)
        with self.assertRaises(adapter.LayaUnavailableError):
            real.decide("x", 1)

    def test_no_laya_import_at_module_load(self):
        self.assertNotIn("laya", sys.modules)


class SelfConsistencyTest(unittest.TestCase):
    def _answer(self, **kw):
        base = adapter.FakeLayaAdapter().decide("generic request", 1)
        for k, v in kw.items():
            setattr(base, k, v)
        return base

    def test_nan_rejected(self):
        a = self._answer(confidence=float("nan"))
        self.assertFalse(adapter.answer_is_self_consistent(a))

    def test_distribution_sum_rejected(self):
        a = self._answer(qos_probabilities={"QI_BALANCED": 0.5, "QI_CRITICAL": 0.2,
                                            "QI_STREAM_LIKE": 0.1, "QI_LOW_COST": 0.1,
                                            "QI_FAILOVER": 0.05})
        self.assertFalse(adapter.answer_is_self_consistent(a))

    def test_unknown_profile_rejected(self):
        a = self._answer(qos_profile="QI_MAGIC", qos_probabilities={"QI_MAGIC": 1.0})
        self.assertFalse(adapter.answer_is_self_consistent(a))

    def test_score_out_of_range_rejected(self):
        a = self._answer(complexity_expected=7.5)
        self.assertFalse(adapter.answer_is_self_consistent(a))


class RulesBaselineTest(unittest.TestCase):
    def test_rules_qos_routing(self):
        self.assertEqual(rules_baseline.predict("security audit of the service").qos_profile,
                         "QI_CRITICAL")
        self.assertEqual(rules_baseline.predict("streaming ao vivo").qos_profile, "QI_STREAM_LIKE")
        self.assertEqual(rules_baseline.predict("traduza o documento").qos_profile, "QI_BALANCED")

    def test_rules_capabilities(self):
        p = rules_baseline.predict("refactor the rust module and add unit tests")
        self.assertGreater(p.capability_probabilities["rust_refactor"], 0.5)
        self.assertGreater(p.capability_probabilities["unit_testing"], 0.5)

    def test_rules_planning(self):
        self.assertGreater(
            rules_baseline.predict("first do X and then do Y").needs_planning_probability, 0.5)
        self.assertLess(rules_baseline.predict("say hello").needs_planning_probability, 0.5)

    def test_rules_distributions_sum_one(self):
        p = rules_baseline.predict("urgent distributed rust refactor and audit")
        self.assertAlmostEqual(sum(p.qos_probabilities.values()), 1.0, places=6)
        self.assertAlmostEqual(sum(p.priority_probabilities), 1.0, places=6)


if __name__ == "__main__":
    unittest.main()
