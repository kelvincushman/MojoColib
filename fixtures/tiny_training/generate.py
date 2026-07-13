#!/usr/bin/env python3
"""Generate the tiny offline model/data fixture without third-party packages."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from collections.abc import Iterable
from pathlib import Path


ROOT = Path(__file__).resolve().parent
SEED = 1729
CONTEXT_LENGTH = 32
SPECIAL_TOKENS = ["<pad>", "<bos>", "<eos>", "<unk>"]
TOKENS = SPECIAL_TOKENS + list("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ01234567")
GENERATED_NAMES = (
    "model_config.json",
    "model.safetensors",
    "vocab.json",
    "train.jsonl",
    "eval.jsonl",
    "recipe.json",
    "expected.json",
)


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n").encode()


def jsonl_bytes(records: Iterable[dict[str, object]]) -> bytes:
    return b"".join(
        (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode()
        for record in records
    )


def deterministic_float(name: str, index: int, scale: float = 0.02) -> float:
    material = f"{SEED}:{name}:{index}".encode()
    integer = int.from_bytes(hashlib.sha256(material).digest()[:4], "little")
    unit = integer / 0xFFFFFFFF
    return (unit * 2.0 - 1.0) * scale


def tensor_bytes(name: str, shape: tuple[int, ...], *, ones: bool = False) -> bytes:
    count = 1
    for dimension in shape:
        count *= dimension
    values = (1.0 for _ in range(count)) if ones else (
        deterministic_float(name, index) for index in range(count)
    )
    return b"".join(struct.pack("<f", value) for value in values)


def tensors() -> list[tuple[str, tuple[int, ...], bytes]]:
    values: list[tuple[str, tuple[int, ...], bytes]] = []

    def add(name: str, shape: tuple[int, ...], *, ones: bool = False) -> None:
        values.append((name, shape, tensor_bytes(name, shape, ones=ones)))

    add("token_embedding.weight", (64, 32))
    add("position_embedding.weight", (32, 32))
    for layer in range(2):
        prefix = f"layers.{layer}"
        add(f"{prefix}.attention_norm.weight", (32,), ones=True)
        add(f"{prefix}.attention_norm.bias", (32,))
        for projection in ("q_proj", "k_proj", "v_proj", "o_proj"):
            add(f"{prefix}.{projection}.weight", (32, 32))
        add(f"{prefix}.ffn_norm.weight", (32,), ones=True)
        add(f"{prefix}.ffn_norm.bias", (32,))
        add(f"{prefix}.up_proj.weight", (64, 32))
        add(f"{prefix}.down_proj.weight", (32, 64))
        for projection in ("q_proj", "v_proj"):
            add(f"{prefix}.{projection}.lora_a", (2, 32))
            add(f"{prefix}.{projection}.lora_b", (32, 2))
    add("final_norm.weight", (32,), ones=True)
    add("final_norm.bias", (32,))
    return values


def safetensors_bytes() -> tuple[bytes, int]:
    header: dict[str, object] = {
        "__metadata__": {
            "format": "pt",
            "generator": "mojocolib-tiny-v1",
            "seed": str(SEED),
        }
    }
    data = bytearray()
    parameter_count = 0
    for name, shape, encoded in tensors():
        start = len(data)
        data.extend(encoded)
        end = len(data)
        header[name] = {"data_offsets": [start, end], "dtype": "F32", "shape": list(shape)}
        parameter_count += end // 4 - start // 4

    encoded_header = json.dumps(header, sort_keys=True, separators=(",", ":")).encode()
    encoded_header += b" " * (-len(encoded_header) % 8)
    return struct.pack("<Q", len(encoded_header)) + encoded_header + data, parameter_count


def records(prefix: str, count: int, offset: int) -> list[dict[str, object]]:
    generated = []
    alphabet = "abcdefghABCDEFGH01234567"
    for index in range(count):
        value = "".join(alphabet[(index * 3 + position + offset) % len(alphabet)] for position in range(8))
        generated.append(
            {
                "id": f"{prefix}-{index + 1:02d}",
                "input": f"copy{value}",
                "target": value,
                "task": "copy_sequence",
            }
        )
    return generated


def tokenise(text: str, vocabulary: dict[str, int]) -> list[int]:
    ids = [vocabulary["<bos>"]]
    ids.extend(vocabulary.get(character, vocabulary["<unk>"]) for character in text)
    ids.append(vocabulary["<eos>"])
    return ids[:CONTEXT_LENGTH]


def generated_files() -> dict[str, bytes]:
    if len(TOKENS) != 64 or len(set(TOKENS)) != 64:
        raise AssertionError("vocabulary must contain 64 unique tokens")
    vocabulary = {token: index for index, token in enumerate(TOKENS)}
    train = records("train", 32, 0)
    evaluation = records("eval", 8, 11)
    weights, parameter_count = safetensors_bytes()
    if parameter_count > 100_000:
        raise AssertionError("model exceeds 100,000 parameters")

    config = {
        "architecture": "MojoColibTinyCausalTransformer",
        "attention_heads": 4,
        "context_length": CONTEXT_LENGTH,
        "dropout": 0.0,
        "dtype": "float32",
        "hidden_size": 32,
        "intermediate_size": 64,
        "layers": 2,
        "parameter_count": parameter_count,
        "tie_word_embeddings": True,
        "vocab_size": 64,
    }
    recipe = {
        "schema_version": 1,
        "seed": SEED,
        "backend": "pytorch_cpu",
        "offline_required": True,
        "deterministic_algorithms": True,
        "micro_batch_size": 2,
        "gradient_accumulation_steps": 2,
        "optimizer_steps": 12,
        "checkpoint_after_step": 5,
        "learning_rate": "0.001000000",
        "optimizer": {
            "name": "adamw",
            "beta1": "0.900000000",
            "beta2": "0.999000000",
            "epsilon": "0.000000010",
            "weight_decay": "0.010000000",
        },
        "scheduler": {"name": "constant"},
        "lora": {
            "alpha": 4,
            "dropout": "0.000000000",
            "rank": 2,
            "targets": ["q_proj", "v_proj"],
        },
        "checkpoint_state": ["adapter", "optimizer", "scheduler", "step", "epoch", "rng"],
    }
    first_text = str(train[0]["input"]) + str(train[0]["target"])
    ids = tokenise(first_text, vocabulary)
    expected = {
        "schema_version": 1,
        "first_train_id": train[0]["id"],
        "first_train_input_ids": ids + [vocabulary["<pad>"]] * (CONTEXT_LENGTH - len(ids)),
        "first_train_attention_mask": [1] * len(ids) + [0] * (CONTEXT_LENGTH - len(ids)),
        "ordered_train_ids": [record["id"] for record in train],
        "ordered_eval_ids": [record["id"] for record in evaluation],
        "weights_sha256": hashlib.sha256(weights).hexdigest(),
        "training_golden_status": "pending_locked_pytorch_environment",
    }
    return {
        "model_config.json": json_bytes(config),
        "model.safetensors": weights,
        "vocab.json": json_bytes(
            {
                "schema_version": 1,
                "tokens": vocabulary,
                "special_token_ids": {"pad": 0, "bos": 1, "eos": 2, "unk": 3},
            }
        ),
        "train.jsonl": jsonl_bytes(train),
        "eval.jsonl": jsonl_bytes(evaluation),
        "recipe.json": json_bytes(recipe),
        "expected.json": json_bytes(expected),
    }


def manifest(files: dict[str, bytes]) -> bytes:
    script_digest = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    entries = [
        {"path": name, "bytes": len(files[name]), "sha256": hashlib.sha256(files[name]).hexdigest()}
        for name in GENERATED_NAMES
    ]
    value = {
        "schema_version": 1,
        "fixture": "mojocolib-tiny-training-v1",
        "seed": SEED,
        "generator_sha256": script_digest,
        "generator_revision": "mojocolib-fixture-v1",
        "license_status": "Apache-2.0-project-owned-synthetic",
        "files": entries,
    }
    return json_bytes(value)


def build() -> dict[str, bytes]:
    files = generated_files()
    files["manifest.json"] = manifest(files)
    total = sum(len(content) for content in files.values())
    if total > 2 * 1024 * 1024:
        raise AssertionError("complete generated fixture exceeds 2 MiB")
    return files


def check(files: dict[str, bytes]) -> list[str]:
    differences = []
    for name, expected in files.items():
        path = ROOT / name
        if not path.is_file():
            differences.append(f"missing {name}")
        elif path.read_bytes() != expected:
            differences.append(f"stale {name}")
    return differences


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if committed outputs are stale")
    arguments = parser.parse_args()
    files = build()
    if arguments.check:
        differences = check(files)
        if differences:
            print("fixture check failed: " + ", ".join(differences), file=sys.stderr)
            return 1
        print(f"fixture check passed ({sum(map(len, files.values()))} bytes)")
        return 0
    for name, content in files.items():
        (ROOT / name).write_bytes(content)
    print(f"generated {len(files)} files ({sum(map(len, files.values()))} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
