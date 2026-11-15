# Plain IDE — Delivery ToDo

## Workspace files and project organization
- [x] Provide a workspace file/folder explorer with actions to create, rename, delete, and organize project files. Explorer operations work on nested project-relative paths, reject unsafe names and overwrite collisions, and recursively remove a deleted folder's contents. Verified by workspace unit tests.
- [x] Support project-wide file search across workspace paths and file contents. Verified by the workspace test suite and the live explorer view.

## Editor and tabs
- [x] Provide a code editor with Plain syntax highlighting, line numbers, indentation support, and common editing shortcuts. The editor supports `Ctrl/Cmd+S` to save and `Ctrl/Cmd+Enter` to run the active `.plain` file. Verified by CodeMirror component tests and the live preview.
- [x] Support tabbed editing for multiple files and show unsaved-change indicators. Verified in the desktop and mobile preview layouts.
- [x] Provide in-file find and replace through CodeMirror's search panel. Verified by editor component tests.

## Run, output, and terminal
- [x] Provide integrated run controls for supported projects with a clear output/console panel. The browser preview runner executes the documented Plain subset and reports output, completion, and errors; it is explicitly not the full Plain compiler. Verified by 5 runtime tests and a browser run producing `sum 1..8 = 36`.
- [x] Provide an integrated terminal for executing project commands within the workspace. Commands are confined to the virtual workspace and support listing, reading, searching, checking, and running Plain files; arbitrary host-shell commands are rejected. Verified by terminal tests and a live terminal run producing the same result.

## Persistence, layout, and accessibility
- [x] Preserve drafts automatically and restore the workspace after refresh. Drafts are stored in this browser's local storage and validated on restore; they are not synchronized to other devices. Verified by persistence tests.
- [x] Provide a responsive, accessible interface with resizable editor, explorer, console, and terminal panels. Explorer and bottom-panel separators support keyboard resizing, and the workbench has desktop and mobile layouts. Verified with desktop/mobile preview captures.

## Verification and delivery
- [x] Add automated tests covering core workspace, editor, persistence, and run-flow behavior; run the tests and build checks. Final run: 7 test files, 27 tests passed; strict TypeScript check and production build passed; route manifest returned HTTP 200; browser verified Run, Ctrl+Enter, and terminal execution.
- [x] Push the completed project to a separate private GitHub repository: [MR-CODESPIKE/plain-ide](https://github.com/MR-CODESPIKE/plain-ide). The project remains on its Manus-managed canonical repository as well.
