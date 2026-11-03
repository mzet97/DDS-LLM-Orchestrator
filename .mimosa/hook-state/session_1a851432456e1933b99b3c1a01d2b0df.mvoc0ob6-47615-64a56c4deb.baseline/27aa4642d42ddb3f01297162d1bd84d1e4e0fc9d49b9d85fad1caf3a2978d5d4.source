"""Cross-language digest parity (T-900-M2-02).

Reproduz em Python os 16 vetores golden congelados em
`crates/dds-contract/idl/WorkflowPlanning.digest_vectors.json` — os MESMOS
hashes que o teste Rust `digest_vectors_reprove_in_rust` verifica. Prova que
o encoder do worker produz exatamente a preimage do contrato.
"""

import json
import sys
import unittest
from pathlib import Path

WORKER = Path(__file__).resolve().parents[1]
REPO = WORKER.parent.parent / "crates" / "dds-contract" / "idl"
sys.path.insert(0, str(WORKER / "src"))

from laya_worker.digest import digest_typed


class DigestParityTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.vectors = json.loads(
            (REPO / "WorkflowPlanning.digest_vectors.json").read_text(encoding="utf-8")
        )["vectors"]

    def test_all_sixteen_golden_vectors_reproduce(self):
        self.assertEqual(len(self.vectors), 16)
        for vec in self.vectors:
            with self.subTest(vector=vec["id"]):
                sha = digest_typed(vec["value"], skip_field=vec.get("skip_field") or None)
                self.assertEqual(sha, vec["sha256"])

    def test_decision_vector_is_the_worker_shape(self):
        d = next(v for v in self.vectors if v["id"] == "decision-full")
        self.assertEqual(d["sha256"], "2841a96f2c79cd39e643502c268b8afe98f2c702acc28ad4a854bd2720047a06")
        self.assertEqual(d["skip_field"], "payload_hash")


if __name__ == "__main__":
    unittest.main()
