"""Unit tests do adapter (T-900-M2-01; T-904-M2): sem pesos, sem rede, sem download."""

import hashlib
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

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


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class LocalCheckpointTest(unittest.TestCase):
    """T-904-M2: modo local explícito (checkpoint_path) — fixtures pequenos."""

    REQUIRED = adapter.CHECKPOINT_REQUIRED_FILES

    def _make_checkpoint(self, tmp: str, weights: bytes = b"w" * 128,
                         complete: bool = True) -> tuple[str, str]:
        """Diretório de checkpoint mínimo; devolve (dir, sha256 dos pesos)."""
        d = Path(tmp) / "laya-t900-fixture"
        d.mkdir()
        (d / "model.safetensors").write_bytes(weights)
        if complete:
            for rel in self.REQUIRED:
                if rel != "model.safetensors":
                    p = d / rel
                    p.parent.mkdir(parents=True, exist_ok=True)
                    p.write_text("{}")
        return str(d), _sha256_bytes(weights)

    def _fake_loader(self, answers: dict | None = None):
        calls = []

        def fake_load(path, expected_sha256=None):
            calls.append({"path": path, "expected_sha256": expected_sha256,
                          "hf_offline": os.environ.get("HF_HUB_OFFLINE"),
                          "tf_offline": os.environ.get("TRANSFORMERS_OFFLINE")})
            canned = answers or {
                "qos_profile": {"choice": "QI_CRITICAL",
                                "probabilities": {"QI_CRITICAL": 0.9, "QI_BALANCED": 0.1}},
                "needs_planning": {"noul": 0.8},
                "cap__unit_testing": {"noul": 0.9},
                "cap__research": {"noul": 0.1},
                "complexity": {"score": 2.0},
                "urgency": {"score": 1.0, "probabilities": {"0": 0.2, "1": 0.7, "2": 0.1}},
            }
            return type("FakeAgent", (), {
                "system_one": staticmethod(lambda ask, qs: {"answers": canned}),
                "load_calibration": staticmethod(lambda p: None),
            })()

        return fake_load, calls

    def _decide_with(self, tmp, *, pin=None, weights=b"w" * 128, complete=True,
                     model_id=None, calibration_path=None):
        import time as _time

        d, real = self._make_checkpoint(tmp, weights=weights, complete=complete)
        fake_load, calls = self._fake_loader()
        deadline = _time.time_ns() + 60_000_000_000  # prazo futuro (adapter real checa)
        with mock.patch.object(adapter, "_laya_load", fake_load):
            a = adapter.RealLayaAdapter(
                checkpoint_path=d,
                checkpoint_sha256=pin or real,
                model_id=model_id,
                calibration_path=calibration_path,
            )
            ans = a.decide("urgent production incident", deadline)
        return a, ans, calls, real

    # T-904-M2-01
    def test_accepts_local_checkpoint_with_correct_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            a, ans, calls, real = self._decide_with(tmp)
            self.assertEqual(len(calls), 1)
            self.assertTrue(calls[0]["path"].endswith("laya-t900-fixture"))
            self.assertEqual(calls[0]["expected_sha256"], {"model.safetensors": real})
            self.assertEqual(ans.checkpoint_revision, real)
            self.assertEqual(ans.model_id, "local:laya-t900-fixture")
            self.assertTrue(adapter.answer_is_self_consistent(ans))

    # T-904-M2-02
    def test_rejects_wrong_hash_without_loading(self):
        with tempfile.TemporaryDirectory() as tmp:
            fake_load, calls = self._fake_loader()
            d, real = self._make_checkpoint(tmp)
            wrong = "b" * 64
            with mock.patch.object(adapter, "_laya_load", fake_load):
                with self.assertRaises(adapter.CheckpointHashMismatchError):
                    adapter.RealLayaAdapter(checkpoint_path=d, checkpoint_sha256=wrong)
            self.assertEqual(calls, [])  # load NUNCA tentado

    # T-904-M2-03
    def test_rejects_missing_path(self):
        fake_load, calls = self._fake_loader()
        with mock.patch.object(adapter, "_laya_load", fake_load):
            with self.assertRaises(adapter.CheckpointMissingError):
                adapter.RealLayaAdapter(checkpoint_path="/nonexistent/dir-t904",
                                        checkpoint_sha256="a" * 64)
        self.assertEqual(calls, [])

    # T-904-M2-04
    def test_rejects_incomplete_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            fake_load, calls = self._fake_loader()
            d, real = self._make_checkpoint(tmp, complete=False)
            with mock.patch.object(adapter, "_laya_load", fake_load):
                with self.assertRaises(adapter.CheckpointIncompleteError) as ctx:
                    adapter.RealLayaAdapter(checkpoint_path=d, checkpoint_sha256=real)
            missing = str(ctx.exception)
            for rel in self.REQUIRED:
                if rel != "model.safetensors":
                    self.assertIn(rel, missing)
            self.assertEqual(calls, [])

    # T-904-M2-05
    def test_local_mode_never_loads_remote_id_and_goes_offline(self):
        with tempfile.TemporaryDirectory() as tmp:
            _, _, calls, _ = self._decide_with(tmp)
            self.assertEqual(len(calls), 1)
            self.assertNotIn("convaiinnovations/laya", calls[0]["path"])
            self.assertEqual(calls[0]["hf_offline"], "1")
            self.assertEqual(calls[0]["tf_offline"], "1")

    # T-904-M2-06
    def test_identity_comes_from_verified_checkpoint(self):
        with tempfile.TemporaryDirectory() as tmp:
            a, ans, _, real = self._decide_with(tmp, model_id="local:custom-name")
            self.assertEqual(ans.checkpoint_revision, real)
            self.assertEqual(ans.model_id, "local:custom-name")

    # T-904-M2-07 (reforço; o teste legado de import lazy continua valendo)
    def test_laya_not_imported_at_module_load(self):
        self.assertNotIn("laya", sys.modules)

    # T-904-M2-08
    def test_calibration_pin_binds_to_loaded_calibration_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            calib = Path(tmp) / "calib.json"
            calib.write_text('{"version": 2}')
            a, ans, _, _ = self._decide_with(tmp, calibration_path=str(calib))
            self.assertEqual(ans.calibration_revision, _sha256_bytes(calib.read_bytes()))

    def test_without_calibration_stays_empty_shadow(self):
        with tempfile.TemporaryDirectory() as tmp:
            _, ans, _, _ = self._decide_with(tmp)
            self.assertEqual(ans.calibration_revision, "")


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
