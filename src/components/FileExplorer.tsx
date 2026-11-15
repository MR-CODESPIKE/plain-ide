import { useMemo, useState, type FormEvent } from 'react';
import { FileCode2, FilePlus2, Folder, FolderOpen, FolderPlus, Search, Trash2, Pencil, X } from 'lucide-react';
import type { WorkspaceEntry } from '../types';
import { baseName, searchWorkspace } from '../lib/workspace';

type TreeNode = { path: string; name: string; kind: 'file' | 'folder'; children: TreeNode[] };
type DialogMode = 'create-file' | 'create-folder' | 'rename' | 'delete';

function makeTree(entries: WorkspaceEntry[]): TreeNode {
  const root: TreeNode = { path: '', name: 'workspace', kind: 'folder', children: [] };
  const nodes = new Map<string, TreeNode>([['', root]]);
  for (const entry of entries) {
    const parts = entry.path.split('/');
    let parent = root;
    for (let index = 0; index < parts.length; index += 1) {
      const path = parts.slice(0, index + 1).join('/');
      let node = nodes.get(path);
      if (!node) {
        node = { path, name: parts[index], kind: index === parts.length - 1 ? entry.kind : 'folder', children: [] };
        nodes.set(path, node);
        parent.children.push(node);
      }
      if (index === parts.length - 1) node.kind = entry.kind;
      parent = node;
    }
  }
  const sort = (node: TreeNode) => {
    node.children.sort((a, b) => a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === 'folder' ? -1 : 1);
    node.children.forEach(sort);
  };
  sort(root);
  return root;
}

interface Props {
  entries: WorkspaceEntry[];
  activePath: string;
  onOpen: (path: string) => void;
  onCreate: (path: string, kind: 'file' | 'folder') => void;
  onRename: (path: string, name: string) => void;
  onDelete: (path: string) => void;
  onError: (message: string) => void;
}

export function FileExplorer({ entries, activePath, onOpen, onCreate, onRename, onDelete, onError }: Props) {
  const [query, setQuery] = useState('');
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(['', 'src', 'examples']));
  const [selectedPath, setSelectedPath] = useState('');
  const [dialog, setDialog] = useState<DialogMode | null>(null);
  const [dialogValue, setDialogValue] = useState('');
  const tree = useMemo(() => makeTree(entries), [entries]);
  const matches = useMemo(() => searchWorkspace(entries, query), [entries, query]);
  const selected = entries.find((entry) => entry.path === selectedPath);

  function beginDialog(mode: DialogMode) {
    if ((mode === 'rename' || mode === 'delete') && !selected) return;
    setDialog(mode);
    setDialogValue(mode === 'rename' && selected ? baseName(selected.path) : mode === 'create-file' ? 'src/untitled.plain' : mode === 'create-folder' ? 'src/new-folder' : '');
  }

  function submitDialog(event: FormEvent) {
    event.preventDefault();
    if (!dialog) return;
    try {
      if (dialog === 'create-file') onCreate(dialogValue.trim(), 'file');
      else if (dialog === 'create-folder') onCreate(dialogValue.trim(), 'folder');
      else if (dialog === 'rename' && selected) onRename(selected.path, dialogValue.trim());
      else if (dialog === 'delete' && selected) onDelete(selected.path);
      setDialog(null);
      setSelectedPath('');
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    }
  }

  function toggleFolder(path: string) {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path); else next.add(path);
      return next;
    });
  }

  function renderNodes(nodes: TreeNode[], depth: number): React.ReactNode {
    return nodes.map((node) => {
      const isOpen = expanded.has(node.path);
      const isActive = node.path === activePath;
      const isSelected = node.path === selectedPath;
      return (
        <div key={node.path}>
          <div className={`tree-row ${isActive ? 'is-active' : ''} ${isSelected ? 'is-selected' : ''}`} style={{ '--depth': depth } as React.CSSProperties}>
            {node.kind === 'folder' ? (
              <button className="tree-item" type="button" aria-expanded={isOpen} onClick={() => { setSelectedPath(node.path); toggleFolder(node.path); }}>
                {isOpen ? <FolderOpen size={15} /> : <Folder size={15} />}<span>{node.name}</span>
              </button>
            ) : (
              <button className="tree-item" type="button" aria-current={isActive ? 'page' : undefined} onClick={() => { setSelectedPath(node.path); onOpen(node.path); }}>
                <FileCode2 size={15} /><span>{node.name}</span>
                {entries.find((entry) => entry.path === node.path)?.kind === 'file' && node.name.endsWith('.plain') && <i className="file-dot" aria-label="Plain source" />}
              </button>
            )}
          </div>
          {node.kind === 'folder' && isOpen && renderNodes(node.children, depth + 1)}
        </div>
      );
    });
  }

  return (
    <aside className="explorer" aria-label="Project explorer">
      <div className="panel-heading">
        <span>EXPLORER</span>
        <div className="icon-actions">
          <button className="icon-button" type="button" aria-label="New file" title="New file" onClick={() => beginDialog('create-file')}><FilePlus2 size={16} /></button>
          <button className="icon-button" type="button" aria-label="New folder" title="New folder" onClick={() => beginDialog('create-folder')}><FolderPlus size={16} /></button>
          <button className="icon-button" type="button" aria-label="Rename selected" title="Rename selected" disabled={!selected} onClick={() => beginDialog('rename')}><Pencil size={15} /></button>
          <button className="icon-button danger-hover" type="button" aria-label="Delete selected" title="Delete selected" disabled={!selected} onClick={() => beginDialog('delete')}><Trash2 size={15} /></button>
        </div>
      </div>
      <label className="project-search">
        <Search size={14} aria-hidden="true" />
        <input aria-label="Search project files" placeholder="Search files and content…" value={query} onChange={(event) => setQuery(event.target.value)} />
        {query && <button type="button" className="search-clear" aria-label="Clear project search" onClick={() => setQuery('')}><X size={13} /></button>}
      </label>
      <div className="tree-scroll">
        {query.trim() ? (
          <div className="search-results" aria-label="Project search results">
            <div className="tree-section-label">{matches.length} MATCH{matches.length === 1 ? '' : 'ES'}</div>
            {matches.map((entry) => <button className={`search-result ${activePath === entry.path ? 'is-active' : ''}`} type="button" key={entry.path} onClick={() => { setSelectedPath(entry.path); onOpen(entry.path); }}><FileCode2 size={14} /><span><b>{entry.path}</b><small>{entry.content.split(/\r?\n/).find((line) => line.toLocaleLowerCase().includes(query.toLocaleLowerCase())) ?? 'Path match'}</small></span></button>)}
            {!matches.length && <p className="empty-note">No matching files.</p>}
          </div>
        ) : <>{renderNodes(tree.children, 0)}{!entries.length && <p className="empty-note">Workspace is empty. Create a file to start.</p>}</>}
      </div>
      <div className="explorer-footer"><span className="live-indicator" /> {entries.filter((entry) => entry.kind === 'file').length} files in this browser workspace</div>
      {dialog && (
        <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setDialog(null); }}>
          <form className="dialog-card" role="dialog" aria-modal="true" aria-labelledby="dialog-title" onSubmit={submitDialog}>
            <h2 id="dialog-title">{dialog === 'create-file' ? 'Create file' : dialog === 'create-folder' ? 'Create folder' : dialog === 'rename' ? 'Rename item' : 'Delete item?'}</h2>
            {dialog === 'delete' ? <p>This removes <code>{selected?.path}</code> and its contents from the local workspace.</p> : <label className="dialog-label">{dialog === 'rename' ? 'New name' : 'Project-relative path'}<input autoFocus value={dialogValue} onChange={(event) => setDialogValue(event.target.value)} placeholder="src/example.plain" /></label>}
            <div className="dialog-actions">
              <button type="button" className="button secondary" onClick={() => setDialog(null)}>Cancel</button>
              <button type="submit" className={`button ${dialog === 'delete' ? 'danger' : 'primary'}`}>{dialog === 'delete' ? 'Delete' : dialog === 'rename' ? 'Rename' : 'Create'}</button>
            </div>
          </form>
        </div>
      )}
    </aside>
  );
}
