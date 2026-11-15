import type { WorkspaceEntry } from '../types';

export function normalizePath(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/').filter(Boolean);
  if (!parts.length || parts.some((part) => part === '.' || part === '..' || part.includes('\0'))) {
    throw new Error('Use a project-relative path without . or .. segments.');
  }
  return parts.join('/');
}

export function parentPath(path: string): string {
  const normalized = normalizePath(path);
  const index = normalized.lastIndexOf('/');
  return index < 0 ? '' : normalized.slice(0, index);
}

export function baseName(path: string): string {
  const normalized = normalizePath(path);
  return normalized.slice(normalized.lastIndexOf('/') + 1);
}

export function ensureParentFolders(entries: WorkspaceEntry[], path: string): WorkspaceEntry[] {
  const parts = normalizePath(path).split('/');
  const result = [...entries];
  for (let i = 1; i < parts.length; i += 1) {
    const folderPath = parts.slice(0, i).join('/');
    if (!result.some((entry) => entry.path === folderPath)) {
      result.push({ path: folderPath, kind: 'folder', content: '' });
    }
  }
  return result;
}

export function createFile(entries: WorkspaceEntry[], path: string): WorkspaceEntry[] {
  const normalized = normalizePath(path);
  if (entries.some((entry) => entry.path === normalized)) throw new Error(`${normalized} already exists.`);
  return [...ensureParentFolders(entries, normalized), { path: normalized, kind: 'file', content: '' }];
}

export function createFolder(entries: WorkspaceEntry[], path: string): WorkspaceEntry[] {
  const normalized = normalizePath(path);
  if (entries.some((entry) => entry.path === normalized)) throw new Error(`${normalized} already exists.`);
  return [...ensureParentFolders(entries, normalized), { path: normalized, kind: 'folder', content: '' }];
}

export function updateFile(entries: WorkspaceEntry[], path: string, content: string): WorkspaceEntry[] {
  let found = false;
  const next = entries.map((entry) => {
    if (entry.path !== path) return entry;
    if (entry.kind !== 'file') throw new Error(`${path} is a folder.`);
    found = true;
    return { ...entry, content };
  });
  if (!found) throw new Error(`${path} does not exist.`);
  return next;
}

export function renameEntry(entries: WorkspaceEntry[], oldPath: string, newName: string): WorkspaceEntry[] {
  const entry = entries.find((item) => item.path === oldPath);
  if (!entry) throw new Error(`${oldPath} does not exist.`);
  const cleanName = newName.trim();
  if (!cleanName || cleanName === '.' || cleanName === '..' || /[\\/\0]/.test(cleanName)) {
    throw new Error('Enter a valid name without slashes.');
  }
  const nextPath = parentPath(oldPath) ? `${parentPath(oldPath)}/${cleanName}` : cleanName;
  const affected = entries.filter((item) => item.path === oldPath || item.path.startsWith(`${oldPath}/`));
  const replacements = affected.map((item) => ({ ...item, path: nextPath + item.path.slice(oldPath.length) }));
  const affectedPaths = new Set(affected.map((item) => item.path));
  const replacementPaths = new Set(replacements.map((item) => item.path));
  if (entries.some((item) => !affectedPaths.has(item.path) && replacementPaths.has(item.path))) {
    throw new Error('That name would overwrite an existing file or folder.');
  }
  return entries.filter((item) => !affectedPaths.has(item.path))
    .concat(replacements)
    .sort((a, b) => a.path.localeCompare(b.path));
}

export function deleteEntry(entries: WorkspaceEntry[], path: string): WorkspaceEntry[] {
  if (!entries.some((entry) => entry.path === path)) return entries;
  return entries.filter((entry) => entry.path !== path && !entry.path.startsWith(`${path}/`));
}

export function listChildren(entries: WorkspaceEntry[], folder = ''): WorkspaceEntry[] {
  return entries.filter((entry) => parentPath(entry.path) === folder)
    .sort((a, b) => a.kind === b.kind ? a.path.localeCompare(b.path) : a.kind === 'folder' ? -1 : 1);
}

export function searchWorkspace(entries: WorkspaceEntry[], query: string): WorkspaceEntry[] {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return [];
  return entries.filter((entry) => entry.kind === 'file' &&
    (entry.path.toLocaleLowerCase().includes(needle) || entry.content.toLocaleLowerCase().includes(needle)));
}

export function findFile(entries: WorkspaceEntry[], path: string): WorkspaceEntry | undefined {
  return entries.find((entry) => entry.path === path && entry.kind === 'file');
}
