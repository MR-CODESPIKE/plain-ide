# Plain IDE

A browser-based workbench for editing `.plain` files. It includes a nested workspace explorer, tabs, project search, a CodeMirror editor with Plain token colors and line numbers, in-file find/replace, a resizable output/terminal panel, and local draft restoration.

## Run locally

```sh
pnpm install --frozen-lockfile
pnpm dev
```

Open the Vite address shown in the terminal. The project uses pnpm 11.25.0, pinned in `package.json`.

## Test and build

```sh
pnpm test
pnpm build
```

The Vitest suite covers workspace operations, the CodeMirror editor and shortcuts, Plain token categories, local persistence, find/replace, terminal commands, and interpreter execution.

## Use

- **Create file / folder:** use the explorer toolbar. Paths are relative to the virtual workspace; Plain source uses `.plain`.
- **Rename / delete:** select an explorer item, then use its toolbar action. Deleting a folder also removes its contents.
- **Tabs:** click a file in the explorer to open it; the dot marks a draft not explicitly saved. Changes are still auto-preserved locally.
- **Find / replace:** use Find or `Ctrl/Cmd+F`; CodeMirror's search panel includes find and replace.
- **Save:** `Ctrl/Cmd+S`. Drafts are stored in this browser's local storage and restored after refresh. They are not uploaded or synchronized to other devices; clearing this browser's site data removes them.
- **Run / check:** use `Ctrl/Cmd+Enter` to run the active file or the Run button. The preview runner accepts `.plain` files and runs a limited, safe browser subset: `LET`, `SET`, `SAY`, `IF`/`ELSE`, counted `REPEAT`, arithmetic, comparisons, strings, and booleans. It is not a replacement for the full Plain compiler.
- **Terminal:** this is a virtual workspace command runner, not an operating-system shell. It supports `help`, `pwd`, `ls`, `cat`, `find`, `plain check`, `plain run`, and `clear`; it cannot execute arbitrary system commands.

The web app is intentionally static and does not include sign-in, shared cloud files, or durable server-side project storage. The file-storage power-up was not selected in the approved blueprint.


## Full native compiler and offline mode

The browser preview remains a limited subset. For full Plain language support, use the local Rust/LLVM compiler bridge in [`NATIVE_COMPILER.md`](NATIVE_COMPILER.md). The supplied Plain Ecosystem compiler, runtime, and ML sources are included under `native/plain-ecosystem/`.

- Windows: run `scripts/setup-windows.ps1` after installing Rust, LLVM 18, Clang, Node.js, and Visual Studio C++ Build Tools.
- Android: use Termux with LLVM 18 (`scripts/setup-termux.sh`), or Ubuntu 24.04 in Termux proot (`scripts/setup-android-ubuntu-proot.sh`).
- Once the compiler/toolchain is built and the PWA shell has been cached, editing, checking, building, and running can work without internet.

See [`NATIVE_COMPILER.md`](NATIVE_COMPILER.md) for setup, offline prerequisites, security limits, and platform caveats. Native Plain programs are not sandboxed.
