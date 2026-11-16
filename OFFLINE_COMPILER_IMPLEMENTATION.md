# Offline compiler implementation report

## Source used

The native toolchain is based on the supplied `plain-ecosystem-fixed.zip` source tree. The integration includes its Rust compiler (`compiler/`), C-compatible runtime (`runtime/`), ML library (`ml/`), language docs, examples, and Cargo lockfile. The compiler's existing front end (lexer, parser, semantic checker), LLVM code generator, runtime calls, and ML library remain the source of language semantics.

## Implemented in this IDE repository

1. Added `native/plain-ecosystem/` containing the actual compiler/runtime/ML source.
2. Added `native/bridge/server.mjs`, a local Node bridge that stages the IDE workspace and invokes the compiler for `check`, `run`, and `build`.
3. Added a random bearer token, loopback-only binding, localhost CORS allowlist, request-size/file-count limits, path traversal checks, temporary project staging, and cleanup.
4. Added stdin forwarding for Plain programs using `READ`.
5. Added a native compiler connection flow to the IDE. Run and Check use the native compiler when connected; otherwise the IDE labels and uses the limited browser preview. Native failure does not silently fall back to the preview.
6. Added Build, which downloads the native executable produced by the compiler.
7. Added a PWA manifest and service worker for caching the IDE shell and static assets.
8. Added setup scripts for Windows/MSVC, Android Termux, and Android Ubuntu 24.04 proot.
9. Adjusted the compiler to build runtime/ML libraries in Cargo offline mode during subsequent runs and to select MSVC `.lib` names / `.exe` output on Windows.

## Verification performed in this environment

- `node --test native/bridge/server.test.mjs`: 3 passed.
- Native bridge manual integration with a fake compiler: health, authenticated Check, Run/stdin, Build artifact response, and path-traversal rejection all behaved as expected.
- `node --check` passed for the bridge and service worker.
- `package.json` and `manifest.webmanifest` parse as valid JSON.
- TypeScript/TSX syntax transpilation: no syntax diagnostics.

## Not verified here

This environment has no Rust/Cargo installation, no LLVM 18, and no project `node_modules`; network access to the npm registry failed. Therefore:

- The Rust/LLVM compiler was not built here.
- Windows/MSVC native compilation and linking were not executed.
- Android ARM64 native compilation was not executed.
- The full Vitest suite and production Vite build were not executed.

This is an integration implementation and reproducible target-platform setup, not a claim that ready-to-run Windows and Android compiler binaries have already been produced. The first native setup needs internet access to install LLVM/Rust/Node dependencies and fetch Cargo crates. After setup, the compiler's runtime/ML builds are configured for Cargo offline mode, and the IDE shell can be cached as a PWA.

## Known constraints

- The upstream compiler is pinned to LLVM 18 through Inkwell. LLVM major versions other than 18 are not accepted by the setup scripts.
- Android direct Termux setup requires an LLVM 18 package. If unavailable, use the Ubuntu proot route; it consumes significant storage and RAM.
- Windows requires the MSVC C++ linker and Windows SDK in addition to LLVM 18, Rust, and Node.
- Native Plain programs execute with the user's OS permissions. The bridge is not a sandbox and must remain on loopback.
- The browser PWA cannot itself launch native binaries; the local bridge is required for full compiler compatibility.
