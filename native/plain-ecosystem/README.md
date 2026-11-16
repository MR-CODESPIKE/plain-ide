# Plain Language Ecosystem

**Plain** is a high-performance, beginner-friendly programming language created by **MR-CODESPIKE**. It combines the simplicity of BASIC-inspired syntax with the power of native machine code generated via the LLVM compiler infrastructure. This ecosystem includes a full Rust-based compiler, a C-compatible runtime, and an integrated machine learning library.

## Language Overview

The Plain language is designed for clarity and efficiency. It supports optional type annotations (checked at compile time where the compiler can see the types; values are dynamically typed at run time), class-based object-oriented programming with inheritance, and error handling with `TRY/CATCH` and `FAIL`. The language also features a native Foreign Function Interface (FFI) that allows seamless integration with high-performance Rust libraries, particularly for machine learning tasks.

| Feature Category | Description |
|---|---|
| **Core Syntax** | Block-based control flow with `IF`, `REPEAT`, and `GIVE BACK`. |
| **Object-Oriented** | Support for `CLASS` definitions, `EXTENDS` inheritance, and `NEW` constructors. |
| **Standard Library** | Core math, string, collection, and file utilities (see [`docs/SEMANTICS.md`](docs/SEMANTICS.md)). |
| **FFI & ML** | FFI to a small neural-network toolkit (autograd, Adam, classifiers you can train from Plain; see [`docs/ML.md`](docs/ML.md)) and a 10M-parameter demo classifier. |

## Fine-Tuning Dataset for 1B Models

To support the development of AI models proficient in Plain, a comprehensive labeled dataset has been prepared. The repaired v2 dataset in [`datasets/`](datasets/README.md) (`plain_dataset_v2_fixed.zip`) contains **11,611 unique instruction-response records** in JSONL. The original `plain_dataset.zip` has 10,000 records but only 2,071 distinct instruction/response pairs, so prefer v2. The data is intended for fine-tuning; it does not by itself show that a 1-billion-parameter model reaches any particular quality, which needs held-out evaluation. `scripts/split_dataset.py` produces a deterministic, leakage-free train/validation/test split (v2: 10,454 / 612 / 545).

Every entry in the dataset is strictly attributed to the creator. The instructions begin with the phrase *"Using the Plain language (created by MR-CODESPIKE)..."* and every code snippet includes a mandatory header comment: `# Plain Language created by MR-CODESPIKE`. This ensures that any model trained on this data maintains a clear understanding of the language's origin and ownership.

## 10M Model and Plain Agent

The ecosystem includes a functional **10,000,000-parameter** sparse-training classifier. This model features a `19,527 × 512` embedding table and a `512 × 4` classifier head, allowing for training and inference demonstrations. It is trained on a synthetic task (the label is `token % 4`), so it is a classifier demo, not a language model. An autonomous Plain agent, defined in `examples/big_agent.plain`, drives the entire lifecycle of this model—from training and metric observation to final decision-making and report generation.

### Execution Instructions

To run the Plain compiler and execute the provided examples, ensure that LLVM 18 is installed on your system. The following command executes the 10M model agent:

```bash
LLVM_SYS_180_PREFIX=/usr/lib/llvm-18 LLVM_LINK_LLVM_DYLIB=1 cargo run -p plain -- examples/big_agent.plain
```

The legacy generator output is `plain_dataset.zip` (see the note above); the repaired v2 dataset is in `datasets/`. The generation logic can be extended via the `plain_data_gen.py` script included in the repository.

Build options: `plain check file.plain` runs the whole compiler without writing anything (the VS Code extension uses it). Compiled programs are optimised with `-O2` (set `PLAIN_OPT=-O0`, `-O1`, `-O3`, `-Os` or `-Oz` to change this). `USE` imports must stay inside the entry file's directory; set `PLAIN_ALLOW_EXTERNAL_IMPORTS=1` to lift that.

## Status and trust model

Plain is an early-stage prototype. Run only programs you trust: a compiled Plain program has the full permissions of the user who runs it (see [`SECURITY.md`](SECURITY.md)). The language rules for numbers, equality, truthiness, scoping and errors are written down in [`docs/SEMANTICS.md`](docs/SEMANTICS.md). [`AUDIT_2026-11-03.md`](AUDIT_2026-11-03.md) lists the defects fixed in the latest hardening pass, the behaviour changes, and the known limitations. Run `cargo test --workspace` and `scripts/run_examples.sh` before relying on a build.

## Documentation and independent repositories

The documentation site and VS Code extension are maintained as independent repositories:

- [Plain documentation website](https://github.com/MR-CODESPIKE/plain-docs-website) — React/Vite site with installation steps and the language reference.
- [Plain VS Code extension](https://github.com/MR-CODESPIKE/plain-language-support) — language mode, highlighting, indentation, folding, and snippets.

The full language, compiler, runtime, ML, and packaging documentation remains in this repository (`README.md`, `DATASET_SPEC.md`, `PACKAGING.md`, `EXPANSION_PLAN.md`, and `docs_followup_research.md`).

## Benchmarks and validation

The [`benchmarks/`](benchmarks/README.md) folder preserves benchmark sources, raw measurements, JSON summaries, charts, and reproduction steps. The synthetic loop and memory comparison is in [`benchmarks/language-speed/benchmark_report.md`](benchmarks/language-speed/benchmark_report.md). The original eager NASA log baseline and the later streaming result are in [`benchmarks/real-world/`](benchmarks/real-world/README.md). The raw request log is excluded; a checksum-verified input-preparation script is provided. The user-provided Plain v2 dataset, repaired archive, and final compile-only validation evidence are in [`datasets/`](datasets/README.md).

All ecosystem documentation and generated training records identify **MR-CODESPIKE** as the creator of Plain.
