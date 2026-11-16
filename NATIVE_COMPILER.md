# Plain IDE native offline compiler

Plain IDE now has an optional local native compiler bridge. It uses the **actual Rust/LLVM compiler, runtime, and ML library** copied from the supplied Plain Ecosystem source tree. It does not translate Plain programs into the browser preview language.

## Architecture

- The IDE remains a static React/Vite app and can be installed as an offline-capable PWA.
- A small Node.js bridge listens only on `127.0.0.1:4317`.
- The bridge receives the current in-memory workspace, writes it into a temporary project directory, and invokes the real `plain` compiler.
- `Check` invokes `plain check`; `Run` invokes the compiler and executes the resulting native program; `Build` invokes `plain build` and downloads the executable.
- `USE` module imports are preserved because all workspace files are staged together under the temporary project root.
- Cargo is forced into offline mode for compiler runtime/ML builds. The complete source tree is shipped in this repository.

The native compiler is a separate local process because a normal browser/PWA is not permitted to launch arbitrary operating-system executables. The PWA caches its UI; the native bridge supplies full compiler functionality while the device is offline.

## Windows 10/11

Install these prerequisites once while online:

1. Rust stable toolchain (`rustup` and Cargo).
2. LLVM **18.x**, including `llvm-config` and `clang`, with its `bin` directory on `PATH`.
3. Node.js 20 or newer.
4. Visual Studio C++ Build Tools and a Windows SDK (needed by the MSVC linker).

The first build needs an internet connection so Cargo can fetch the Rust crate dependencies. After that, compiler execution builds only the local runtime/ML crates in Cargo offline mode. Then open PowerShell in this repository and run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\setup-windows.ps1
```

The script builds the compiler and workspace libraries locally and starts the bridge. Keep its terminal open. It prints a one-time connection token; in Plain IDE choose **Connect compiler** and paste that token. The token is stored in this browser's local storage.

## Android (Termux, ARM64)

This is the Android-native route; the compiler does not run inside Chrome's JavaScript sandbox.

1. Install Termux from a trusted distribution source and open it.
2. Install the toolchain packages:

```sh
pkg update
pkg install rust llvm clang nodejs
```

3. Confirm `llvm-config --version` reports **18.x**. The compiler's Inkwell dependency explicitly targets LLVM 18; a different major version is not compatible with this source as-is.
4. Copy/clone this project into Termux storage and run (the first build needs an internet connection for Cargo crates):

```sh
bash scripts/setup-termux.sh
```

Keep Termux open while using the IDE. Open the PWA in the same device, choose **Connect compiler**, and paste the token printed by Termux.

> Termux repositories may offer a newer LLVM major version rather than LLVM 18. If `llvm-config` is not 18.x, stop: do not bypass the version check. A matching LLVM 18 build/package is required. This repository does not currently include a prebuilt Android LLVM 18 toolchain.

### Android fallback: Ubuntu in Termux proot

If the Termux package repository does not provide LLVM 18 for your device, use an Ubuntu 24.04 proot (ARM64) where the matching LLVM 18 development packages are available:

```sh
# In Termux
pkg install proot-distro
proot-distro install ubuntu
proot-distro login ubuntu
```

Inside the Ubuntu shell, put/clone this repository (including `native/plain-ecosystem`) under `/root`, then run:

```sh
bash scripts/setup-android-ubuntu-proot.sh
```

This builds and runs the compiler inside the Ubuntu userland. Keep that proot shell open while using the IDE. The local bridge still binds to Android loopback. The first setup downloads OS and Cargo dependencies; subsequent use is offline. This route needs substantial free storage and RAM for LLVM and the Rust build.

## Install the IDE as an offline PWA

Build and host the static IDE with `pnpm install` and `pnpm build` (serve the `dist/` directory over HTTPS, or from localhost). Open it once while online so the service worker can cache the app shell and assets. In Chrome/Edge, use **Install app** / **Add to Home screen**. The PWA can then reopen without internet; the native bridge must also be running locally for full compiler actions.

## Offline behaviour

After the initial toolchain installation/build and one successful online PWA visit:

- The IDE shell and previously cached application assets load offline.
- The local compiler bridge works without a network connection.
- The compiler, runtime, ML library, Cargo dependencies, and Clang/LLVM must already be installed locally.
- `USE` imports work for files included in the IDE workspace.
- Programs can read/write files and use FFI according to the Plain runtime and OS permissions.

The browser cannot cache external fonts anymore; system font fallbacks are used. To refresh the IDE shell, reconnect to the internet and load the latest app once.

## Security model

The bridge binds to loopback only and requires a random bearer token for compilation requests. It accepts only relative workspace paths, caps requests at 2 MiB/300 files, stages sources in a temporary directory, and removes that directory after each request. CORS is limited to localhost origins by default; set `PLAIN_IDE_ORIGINS` to a comma-separated list only if hosting the IDE on a trusted HTTPS origin.

**Important:** Plain is a native language. Running a Plain program gives it the permissions of the logged-in OS user. The bridge is not a sandbox. Only run programs you trust, and do not expose the bridge to a LAN or public network.

## Compatibility notes

- Full language compatibility comes from using the upstream Plain parser, semantic checker, LLVM code generator, runtime, and ML library. The browser preview interpreter remains available only as a clearly labelled fallback when the native bridge is not connected.
- The compiler source was adjusted to use Cargo offline mode for its local runtime/ML builds and to select the expected static-library/output names for Windows MSVC versus Unix/Android.
- This environment did not contain Rust/Cargo or LLVM 18, so Windows and Android native binaries could not be built or executed here. The scripts are build/install paths, not proof of a tested prebuilt binary. Validate on each target device with the ecosystem's examples before production use.
