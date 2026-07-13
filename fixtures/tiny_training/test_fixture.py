from __future__ import annotations

import hashlib
import json
import struct
import unittest
from pathlib import Path

import generate


ROOT = Path(__file__).resolve().parent


class TinyFixtureTests(unittest.TestCase):
    def test_generated_files_match_manifest(self) -> None:
        files = generate.build()
        self.assertEqual(generate.check(files), [])
        manifest = json.loads(files["manifest.json"])
        self.assertEqual(manifest["generator_revision"], "mojocolib-fixture-v1")
        self.assertEqual(manifest["license_status"], "Apache-2.0-project-owned-synthetic")
        self.assertNotIn("generator_commit", manifest)
        for entry in manifest["files"]:
            content = files[entry["path"]]
            self.assertEqual(len(content), entry["bytes"])
            self.assertEqual(hashlib.sha256(content).hexdigest(), entry["sha256"])

    def test_safetensors_offsets_cover_exact_data(self) -> None:
        encoded = (ROOT / "model.safetensors").read_bytes()
        header_length = struct.unpack("<Q", encoded[:8])[0]
        header = json.loads(encoded[8 : 8 + header_length])
        data_length = len(encoded) - 8 - header_length
        tensors = [value for key, value in header.items() if key != "__metadata__"]
        self.assertGreater(len(tensors), 0)
        self.assertEqual(max(tensor["data_offsets"][1] for tensor in tensors), data_length)

    def test_fixture_is_small_and_has_disjoint_splits(self) -> None:
        files = generate.build()
        self.assertLess(sum(map(len, files.values())), 2 * 1024 * 1024)
        train_ids = {json.loads(line)["id"] for line in files["train.jsonl"].splitlines()}
        eval_ids = {json.loads(line)["id"] for line in files["eval.jsonl"].splitlines()}
        self.assertEqual(len(train_ids), 32)
        self.assertEqual(len(eval_ids), 8)
        self.assertTrue(train_ids.isdisjoint(eval_ids))


if __name__ == "__main__":
    unittest.main()
