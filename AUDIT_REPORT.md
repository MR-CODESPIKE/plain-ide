# Plain IDE — Source Audit & Repair Report

Audit date: 2026-11-15
Scope: source code supplied in `plain-ide-main.zip`

## Findings fixed

### 1. Counted-loop non-termination through non-finite numeric values
**Severity: High (availability / denial of service)**

The interpreter's numeric conversion accepted JavaScript `Infinity` when the value was already a number. A sufficiently large numeric literal or overflowing arithmetic expression could therefore produce non-finite loop bounds. The old loop-count calculation could become `NaN`, bypass the range check, and enter a loop whose counter could no longer advance (`Infinity + 1 === Infinity`). In environments without Web Workers this could freeze the UI; in a worker it could consume the runner's timeout.

**Repair:** Numeric conversion now rejects all non-finite values. Counted-loop bounds must be safe whole numbers, loop counts must be safe and within the configured maximum, and iteration uses a bounded index rather than relying on a potentially non-advancing floating-point counter. Regression tests were added.

### 2. Workspace file/folder prefix collisions
**Severity: Medium (workspace integrity)**

Workspace operations could create invalid trees, for example a file at `src` while `src/main.plain` existed, or a child beneath an existing file. This could leave the explorer's implicit tree representation inconsistent with the underlying entries.

**Repair:** Creation now rejects file-as-parent conflicts and file creation at a path that already contains descendants. Rename now rejects exact and ancestor/descendant collisions. Regression tests were added.

### 3. Untrusted local-storage workspace structure
**Severity: Medium (data integrity / robustness)**

Snapshot validation checked basic field types but did not reject duplicate paths, invalid path trees, missing/non-folder ancestors, oversized content, or excessive entry counts. Malformed local storage could therefore restore an inconsistent or excessively large workspace.

**Repair:** Restore validation now limits entry/tab counts, path and file-content sizes, total stored source size, and verifies canonical unique paths and valid folder ancestry. Invalid snapshots fall back to the starter workspace.

### 4. Worker startup and communication errors
**Severity: Low (reliability)**

A synchronous Worker-construction or `postMessage` failure could reject the run promise instead of producing a normal IDE error result. Worker message deserialization errors were not handled.

**Repair:** Worker startup, posting, and message-deserialization failures now resolve to explicit run errors.

## Verification performed

- Strict TypeScript compilation of the core non-UI modules passed using the globally available TypeScript compiler.
- Targeted runtime checks passed for arithmetic errors, excessive loops, overflowing arithmetic, unsafe loop bounds, and the existing sample loop/conditional program.
- Targeted workspace checks passed for nested creation and rejection of file/folder path collisions.
- Targeted persistence checks passed for restoring the starter workspace and rejecting malformed path trees.
- Terminal check confirmed arbitrary shell commands remain rejected.

## Verification not completed

The full Vitest suite and Vite production build could not be run because dependencies are not installed and the environment could not reach the npm registry to download the pinned pnpm version. The repository's existing test suite has been extended, but those added tests still need to be run with `pnpm install --frozen-lockfile`, `pnpm test`, and `pnpm build` in a network-enabled environment.

No live dependency vulnerability scan was possible. The project is a static browser IDE with local-storage persistence; it does not provide account authentication, cloud synchronization, or server-side project storage. The interpreter remains a documented preview subset, not a full Plain compiler.
