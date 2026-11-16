> Historical manifest for the original 2026-10-22 all-in-one source archive. The current code, website, and extension are split into separate repositories.

# Plain Ecosystem Delivery Manifest

Creator: **MR-CODESPIKE**

This package contains the Plain Rust/LLVM compiler, C-compatible runtime, integrated `plain_ml` library, examples, attributed 10,000-record fine-tuning dataset, VS Code extension source and VSIX, and the React/Vite documentation website source and production `dist/` build.

## Validation completed

- `python3 scripts/validate_ecosystem.py` — PASS
- `npm run build` in `docs-website/` — PASS
- `npx @vscode/vsce package --no-dependencies` in `vscode-extension/` — PASS
- `unzip -t plain_dataset.zip` — PASS; 10,000 JSONL records and MR-CODESPIKE attribution verified
- Native Rust/LLVM validation could not be run in this sandbox because `cargo`, `rustc`, `clang`, and `llvm-config` are not installed. `PACKAGING.md` contains the exact commands for a toolchain-equipped machine.
