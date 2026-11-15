import { useEffect, useMemo, useRef, useState } from 'react';
import { Activity, BookOpen, Check, ChevronDown, ChevronRight, Code2, FileCode2, FolderTree, PanelBottom, Play, Save, Search, Settings2, TerminalSquare, X } from 'lucide-react';
import type { EditorView } from '@codemirror/view';
import { openSearchPanel } from '@codemirror/search';
import { Editor } from './components/Editor';
import { FileExplorer } from './components/FileExplorer';
import { OutputPanel, type OutputState } from './components/OutputPanel';
import { TerminalPanel } from './components/TerminalPanel';
import { DEFAULT_SNAPSHOT } from './lib/defaultWorkspace';
import { persistSnapshot, restoreSnapshot, STORAGE_KEY } from './lib/persistence';
import { checkPlain, runPlain } from './lib/plainRuntime';
import { baseName, createFile, createFolder, deleteEntry, findFile, renameEntry, updateFile } from './lib/workspace';
import type { RunResult, WorkspaceEntry, WorkspaceSnapshot } from './types';

function initialSnapshot(): WorkspaceSnapshot {
  try { return restoreSnapshot(window.localStorage, STORAGE_KEY); }
  catch { return structuredClone(DEFAULT_SNAPSHOT); }
}

function clockLabel(): string {
  return new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' }).format(new Date());
}

function runInWorker(source: string): Promise<RunResult> {
  if (typeof Worker === 'undefined') return Promise.resolve(runPlain(source));
  return new Promise((resolve) => {
    const worker = new Worker(new URL('./workers/plainRunner.worker.ts', import.meta.url), { type: 'module' });
    const timeout = window.setTimeout(() => {
      worker.terminate();
      resolve({ ok: false, output: [], error: 'Preview runner timed out.', steps: 0 });
    }, 3000);
    worker.addEventListener('message', (event: MessageEvent<RunResult>) => {
      window.clearTimeout(timeout);
      worker.terminate();
      resolve(event.data);
    }, { once: true });
    worker.addEventListener('error', (event) => {
      window.clearTimeout(timeout);
      worker.terminate();
      resolve({ ok: false, output: [], error: event.message || 'Preview runner could not start.', steps: 0 });
    }, { once: true });
    worker.postMessage({ source });
  });
}

export function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot>(initialSnapshot);
  const [storageStatus, setStorageStatus] = useState('Drafts auto-save in this browser');
  const [output, setOutput] = useState<OutputState>({ title: 'Console ready', lines: [], kind: 'idle' });
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  const [mobileExplorerOpen, setMobileExplorerOpen] = useState(false);
  const [drag, setDrag] = useState<{ kind: 'explorer' | 'bottom'; start: number; initial: number } | null>(null);
  const editorView = useRef<EditorView | null>(null);
  const activeFile = useMemo(() => findFile(snapshot.entries, snapshot.activePath), [snapshot.entries, snapshot.activePath]);
  const openFiles = snapshot.openPaths.map((path) => findFile(snapshot.entries, path)).filter((entry): entry is WorkspaceEntry => Boolean(entry));
  const shortcutMod = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘' : 'Ctrl';

  useEffect(() => {
    try {
      persistSnapshot(window.localStorage, snapshot, STORAGE_KEY);
      setStorageStatus('Draft saved locally');
    } catch {
      setStorageStatus('Browser storage is full — export your work before closing');
    }
  }, [snapshot]);

  useEffect(() => {
    if (!drag) return undefined;
    const move = (event: PointerEvent) => {
      if (drag.kind === 'explorer') {
        const next = Math.min(420, Math.max(180, drag.initial + event.clientX - drag.start));
        setSnapshot((current) => ({ ...current, explorerWidth: next }));
      } else {
        const next = Math.min(480, Math.max(120, drag.initial + drag.start - event.clientY));
        setSnapshot((current) => ({ ...current, bottomHeight: next }));
      }
    };
    const end = () => setDrag(null);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end, { once: true });
    return () => { window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', end); };
  }, [drag]);

  function openFile(path: string) {
    const file = findFile(snapshot.entries, path);
    if (!file) return;
    setSnapshot((current) => ({ ...current, activePath: path, openPaths: current.openPaths.includes(path) ? current.openPaths : [...current.openPaths, path] }));
    setMobileExplorerOpen(false);
  }

  function closeTab(path: string) {
    setSnapshot((current) => {
      const openPaths = current.openPaths.filter((item) => item !== path);
      const activePath = current.activePath === path ? (openPaths[openPaths.length - 1] ?? '') : current.activePath;
      return { ...current, openPaths, activePath, dirtyPaths: current.dirtyPaths.filter((item) => item !== path) };
    });
  }

  function updateContent(path: string, content: string) {
    setSnapshot((current) => ({ ...current, entries: updateFile(current.entries, path, content), dirtyPaths: current.dirtyPaths.includes(path) ? current.dirtyPaths : [...current.dirtyPaths, path] }));
  }

  function saveActive() {
    if (!snapshot.activePath) return;
    setSnapshot((current) => ({ ...current, dirtyPaths: current.dirtyPaths.filter((path) => path !== current.activePath) }));
    setStorageStatus(`Saved ${baseName(snapshot.activePath)} locally`);
  }

  function showError(message: string) {
    setSnapshot((current) => ({ ...current, bottomTab: 'output' }));
    setOutput({ title: 'Workspace action failed', lines: [message], kind: 'error', timestamp: clockLabel() });
  }

  function handleCreate(path: string, kind: 'file' | 'folder') {
    const nextEntries = kind === 'file' ? createFile(snapshot.entries, path) : createFolder(snapshot.entries, path);
    setSnapshot((current) => ({ ...current, entries: nextEntries, ...(kind === 'file' ? { activePath: path, openPaths: [...new Set([...current.openPaths, path])] } : {}) }));
  }

  function handleRename(oldPath: string, name: string) {
    const parent = oldPath.includes('/') ? oldPath.slice(0, oldPath.lastIndexOf('/')) : '';
    const newPath = parent ? `${parent}/${name}` : name;
    const mapPath = (path: string) => path === oldPath || path.startsWith(`${oldPath}/`) ? newPath + path.slice(oldPath.length) : path;
    const entries = renameEntry(snapshot.entries, oldPath, name);
    setSnapshot((current) => ({
      ...current,
      entries,
      openPaths: current.openPaths.map(mapPath),
      activePath: mapPath(current.activePath),
      dirtyPaths: current.dirtyPaths.map(mapPath),
    }));
  }

  function handleDelete(path: string) {
    const entries = deleteEntry(snapshot.entries, path);
    const removed = (candidate: string) => candidate === path || candidate.startsWith(`${path}/`);
    const openPaths = snapshot.openPaths.filter((candidate) => !removed(candidate));
    const activePath = removed(snapshot.activePath) ? (openPaths[openPaths.length - 1] ?? entries.find((entry) => entry.kind === 'file')?.path ?? '') : snapshot.activePath;
    setSnapshot((current) => ({ ...current, entries, openPaths, activePath, dirtyPaths: current.dirtyPaths.filter((candidate) => !removed(candidate)) }));
  }

  async function runFile(path: string): Promise<RunResult> {
    const file = findFile(snapshot.entries, path);
    if (!file) {
      const result = { ok: false, output: [], error: `File not found: ${path}`, steps: 0 };
      setOutput({ title: 'Run failed', lines: [result.error], kind: 'error', timestamp: clockLabel() });
      return result;
    }
    if (!path.endsWith('.plain')) {
      const result = { ok: false, output: [], error: 'The browser preview runner accepts .plain files only.', steps: 0 };
      setOutput({ title: 'Run unavailable', lines: [result.error], kind: 'info', timestamp: clockLabel() });
      return result;
    }
    setOutput({ title: `Running ${baseName(path)}…`, lines: [], kind: 'info' });
    const result = await runInWorker(file.content);
    setOutput({
      title: result.ok ? `${baseName(path)} · completed` : `${baseName(path)} · error`,
      lines: result.ok ? [...result.output, `Finished · ${result.steps} statements`] : [...result.output, result.error ?? 'Run failed'],
      kind: result.ok ? 'success' : 'error',
      timestamp: clockLabel(),
    });
    return result;
  }

  function checkActive() {
    if (!activeFile) return;
    const checked = checkPlain(activeFile.content);
    setSnapshot((current) => ({ ...current, bottomTab: 'output' }));
    setOutput({ title: checked.ok ? `${baseName(activeFile.path)} · check passed` : `${baseName(activeFile.path)} · check failed`, lines: [checked.message], kind: checked.ok ? 'success' : 'error', timestamp: clockLabel() });
  }

  function activateFind() {
    if (!editorView.current) return;
    openSearchPanel(editorView.current);
  }

  function closeActiveTab(event: React.MouseEvent, path: string) {
    event.stopPropagation();
    closeTab(path);
  }

  function beginResize(kind: 'explorer' | 'bottom', event: React.PointerEvent) {
    event.preventDefault();
    setDrag({ kind, start: kind === 'explorer' ? event.clientX : event.clientY, initial: kind === 'explorer' ? snapshot.explorerWidth : snapshot.bottomHeight });
  }

  function resizeByKeyboard(kind: 'explorer' | 'bottom', event: React.KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight' && event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
    event.preventDefault();
    const delta = event.key === 'ArrowLeft' || event.key === 'ArrowUp' ? -16 : 16;
    setSnapshot((current) => kind === 'explorer'
      ? { ...current, explorerWidth: Math.min(420, Math.max(180, current.explorerWidth + delta)) }
      : { ...current, bottomHeight: Math.min(480, Math.max(120, current.bottomHeight + delta)) });
  }

  return (
    <main className="ide-shell" style={{ '--explorer-width': `${snapshot.explorerWidth}px`, '--bottom-height': `${snapshot.bottomHeight}px` } as React.CSSProperties}>
      <header className="topbar">
        <div className="brand-lockup"><span className="brand-mark" aria-hidden="true"><span /><span /></span><span className="brand-name">PLAIN<span>IDE</span></span><span className="brand-divider" /><span className="workspace-label">WORKSPACE</span></div>
        <div className="top-actions">
          <button className="mobile-explorer-button" type="button" aria-label="Toggle explorer" aria-expanded={mobileExplorerOpen} onClick={() => setMobileExplorerOpen((open) => !open)}><FolderTree size={16} /></button>
          <button className="tool-button" type="button" onClick={activateFind} disabled={!activeFile} title="Find and replace (Ctrl/Cmd+F)"><Search size={15} /><span>Find</span></button>
          <button className="tool-button" type="button" onClick={saveActive} disabled={!activeFile} title="Save draft (Ctrl/Cmd+S)"><Save size={15} /><span>Save</span></button>
          <button className="tool-button" type="button" onClick={checkActive} disabled={!activeFile} title="Check supported Plain syntax"><Check size={15} /><span>Check</span></button>
          <button className="run-button" type="button" title={`Run active file (${shortcutMod}+Enter)`} onClick={() => activeFile && void runFile(activeFile.path)} disabled={!activeFile || !activeFile.path.endsWith('.plain')}><Play size={14} fill="currentColor" /><span>Run</span><kbd>{shortcutMod} ↵</kbd></button>
        </div>
        <div className="topbar-status"><span className="connection-dot" /> Local workspace <button className="quiet-icon" type="button" title="Workspace options" aria-label="Workspace options"><Settings2 size={16} /></button></div>
      </header>

      <div className="project-strip"><div className="crumb"><Code2 size={14} /><span>Plain Workspace</span><ChevronRight size={13} /><span>{activeFile?.path ?? 'No file open'}</span></div><div className="project-strip-right"><span className="preview-badge"><span /> PREVIEW SUBSET</span><button className="quiet-icon" type="button" title="Toggle bottom panel" aria-label="Toggle bottom panel" onClick={() => setSnapshot((current) => ({ ...current, bottomHeight: current.bottomHeight ? 0 : 238 }))}><PanelBottom size={15} /></button></div></div>

      <section className={`workspace-layout ${mobileExplorerOpen ? 'show-explorer' : ''}`} aria-label="Plain IDE workspace">
        <FileExplorer entries={snapshot.entries} activePath={snapshot.activePath} onOpen={openFile} onCreate={handleCreate} onRename={handleRename} onDelete={handleDelete} onError={showError} />
        <div className="resize-grip explorer-grip" role="separator" aria-label="Resize project explorer" aria-orientation="vertical" aria-valuemin={180} aria-valuemax={420} aria-valuenow={snapshot.explorerWidth} tabIndex={0} onPointerDown={(event) => beginResize('explorer', event)} onKeyDown={(event) => resizeByKeyboard('explorer', event)} />
        <div className="workbench">
          <div className="tab-strip" role="tablist" aria-label="Open files">
            {openFiles.map((file) => <div key={file.path} className={`file-tab ${file.path === snapshot.activePath ? 'active' : ''}`} role="tab" aria-selected={file.path === snapshot.activePath} tabIndex={0} onClick={() => openFile(file.path)} onKeyDown={(event) => { if (event.target === event.currentTarget && (event.key === 'Enter' || event.key === ' ')) openFile(file.path); }}><FileCode2 size={14} /><span>{baseName(file.path)}</span>{snapshot.dirtyPaths.includes(file.path) && <i className="dirty-dot" aria-label="Unsaved changes" />}<span className="tab-path">{file.path.slice(0, -baseName(file.path).length).replace(/\/$/, '')}</span><button className="tab-close" type="button" aria-label={`Close ${baseName(file.path)}`} onClick={(event) => closeActiveTab(event, file.path)}><X size={13} /></button></div>)}
            {!openFiles.length && <span className="empty-tabs">No open files</span>}
            <div className="tab-spacer" />
            <button className="tab-action" type="button" aria-label="Open find and replace" title="Find and replace" disabled={!activeFile} onClick={activateFind}><Search size={14} /></button>
            <button className="tab-action" type="button" aria-label="Run active file" title="Run active file" disabled={!activeFile || !activeFile.path.endsWith('.plain')} onClick={() => activeFile && void runFile(activeFile.path)}><Play size={13} /></button>
          </div>
          <div className="editor-region">
            {activeFile ? (
              <>
                <div className="editor-breadcrumb"><span className="breadcrumb-muted">{activeFile.path.split('/').slice(0, -1).join(' / ') || 'workspace'}</span><ChevronRight size={13} /><FileCode2 size={14} className="plain-file-icon" /><strong>{baseName(activeFile.path)}</strong><span className="editor-breadcrumb-spacer" /><span className="encoding-label">UTF-8</span><span className="language-label">Plain</span></div>
                <Editor key={activeFile.path} path={activeFile.path} value={activeFile.content} onChange={(content) => updateContent(activeFile.path, content)} onSave={saveActive} onRun={() => void runFile(activeFile.path)} onCursor={(line, column) => setCursor({ line, column })} onView={(view) => { editorView.current = view; }} />
              </>
            ) : <div className="welcome-panel"><div className="welcome-mark"><span>&gt;</span><span>_</span></div><p className="eyebrow">PLAIN LANGUAGE WORKBENCH</p><h1>Write plainly.<br /><em>Run safely.</em></h1><p>Open a file from the explorer or create a new <code>.plain</code> program. Drafts stay in this browser.</p><button className="button primary" type="button" onClick={() => handleCreate('src/untitled.plain', 'file')}>Create a Plain file</button><div className="welcome-shortcuts"><span><kbd>{shortcutMod}</kbd><kbd>S</kbd> save</span><span><kbd>{shortcutMod}</kbd><kbd>F</kbd> find</span><span><kbd>{shortcutMod}</kbd><kbd>↵</kbd> run</span></div></div>}
          </div>
          <div className="resize-grip horizontal-grip" role="separator" aria-label="Resize output and terminal panels" aria-orientation="horizontal" aria-valuemin={120} aria-valuemax={480} aria-valuenow={snapshot.bottomHeight} tabIndex={0} onPointerDown={(event) => beginResize('bottom', event)} onKeyDown={(event) => resizeByKeyboard('bottom', event)} />
          <section className="bottom-panel" aria-label="Output and terminal" style={{ height: snapshot.bottomHeight ? `${snapshot.bottomHeight}px` : 0 }}>
            <div className="bottom-tabs" role="tablist" aria-label="Bottom panel tabs">
              <button role="tab" aria-selected={snapshot.bottomTab === 'output'} className={snapshot.bottomTab === 'output' ? 'active' : ''} type="button" onClick={() => setSnapshot((current) => ({ ...current, bottomTab: 'output' }))}><Activity size={14} /> Output{output.kind === 'error' && <span className="tab-error-dot" />}</button>
              <button role="tab" aria-selected={snapshot.bottomTab === 'terminal'} className={snapshot.bottomTab === 'terminal' ? 'active' : ''} type="button" onClick={() => setSnapshot((current) => ({ ...current, bottomTab: 'terminal' }))}><TerminalSquare size={14} /> Terminal</button>
              <div className="bottom-tab-spacer" />
              <span className="panel-caption">{snapshot.bottomTab === 'terminal' ? 'SAFE WORKSPACE COMMANDS' : output.timestamp ? `LAST RUN · ${output.timestamp}` : 'READY'}</span>
              <button className="quiet-icon" type="button" aria-label="Hide bottom panel" title="Hide panel" onClick={() => setSnapshot((current) => ({ ...current, bottomHeight: 0 }))}><ChevronDown size={15} /></button>
            </div>
            <div className="bottom-content" hidden={snapshot.bottomTab !== 'output'}><OutputPanel output={output} /></div>
            <div className="bottom-content" hidden={snapshot.bottomTab !== 'terminal'}><TerminalPanel entries={snapshot.entries} runPath={runFile} /></div>
          </section>
        </div>
      </section>

      <footer className="statusbar"><div className="status-left"><span className="status-brand">P</span><span>Plain IDE · browser preview</span><span className="status-separator" /><span className="status-auto"><span className="status-led" />{storageStatus}</span></div><div className="status-right"><span>Ln {cursor.line}, Col {cursor.column}</span><span>Spaces: 2</span><span>UTF-8</span><span className="local-tag">LOCAL ONLY</span><button type="button" className="quiet-icon help-button" aria-label="Open quick help" title="Quick help" onClick={() => setOutput({ title: 'Quick help', lines: ['The Run button executes a safe browser preview subset: LET, SET, SAY, IF/ELSE, counted REPEAT, arithmetic, comparisons, strings, and booleans.', 'Terminal commands are confined to the virtual workspace. Type help for commands.', 'For full Plain compatibility, install the Plain compiler and VS Code extension.'], kind: 'info' })}><BookOpen size={14} /></button></div></footer>
      <div className="mobile-run-actions"><button className="button secondary" type="button" onClick={checkActive}><Check size={14} /> Check</button><button className="button primary" type="button" disabled={!activeFile?.path.endsWith('.plain')} onClick={() => activeFile && void runFile(activeFile.path)}><Play size={14} fill="currentColor" /> Run</button></div>
    </main>
  );
}
