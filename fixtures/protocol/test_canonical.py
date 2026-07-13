from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path


FIXTURE = Path(__file__).with_name("canonical_cases.json")


class CanonicalFixtureTests(unittest.TestCase):
    def test_python_matches_v1_golden_bytes(self) -> None:
        document = json.loads(FIXTURE.read_text(encoding="utf-8"))
        self.assertEqual(document["schema_version"], 1)
        for case in document["cases"]:
            with self.subTest(case=case["name"]):
                encoded = json.dumps(
                    case["value"],
                    ensure_ascii=False,
                    separators=(",", ":"),
                    sort_keys=True,
                ).encode("utf-8")
                self.assertEqual(encoded.decode("utf-8"), case["canonical"])
                self.assertEqual(
                    "sha256:" + hashlib.sha256(encoded).hexdigest(),
                    case["sha256"],
                )


if __name__ == "__main__":
    unittest.main()
