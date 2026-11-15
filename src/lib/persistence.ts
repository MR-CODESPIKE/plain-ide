import type { WorkspaceSnapshot } from '../types';
import { DEFAULT_SNAPSHOT } from './defaultWorkspace';

export const STORAGE_KEY = 'plain-ide.workspace.v1';

type StorageLike = Pick<Storage, 'getItem' | 'setItem'>;

function isSnapshot(value: unknown): value is WorkspaceSnapshot {
  if (!value || typeof value !== 'object') return false;
  const item = value as Partial<WorkspaceSnapshot>;
  return Array.isArray(item.entries) && Array.isArray(item.openPaths) &&
    typeof item.activePath === 'string' && Array.isArray(item.dirtyPaths) &&
    typeof item.explorerWidth === 'number' && typeof item.bottomHeight === 'number' &&
    (item.bottomTab === 'output' || item.bottomTab === 'terminal') &&
    item.entries.every((entry) => entry && typeof entry.path === 'string' &&
      (entry.kind === 'file' || entry.kind === 'folder') && typeof entry.content === 'string');
}

export function restoreSnapshot(storage: StorageLike, key = STORAGE_KEY): WorkspaceSnapshot {
  try {
    const raw = storage.getItem(key);
    if (!raw) return structuredClone(DEFAULT_SNAPSHOT);
    const parsed: unknown = JSON.parse(raw);
    if (!isSnapshot(parsed)) return structuredClone(DEFAULT_SNAPSHOT);
    const filePaths = new Set(parsed.entries.filter((entry) => entry.kind === 'file').map((entry) => entry.path));
    const openPaths = [...new Set(parsed.openPaths.filter((path) => filePaths.has(path)))];
    const activePath = filePaths.has(parsed.activePath) ? parsed.activePath : (openPaths[0] ?? '');
    if (activePath && !openPaths.includes(activePath)) openPaths.push(activePath);
    return {
      ...parsed,
      openPaths,
      activePath,
      dirtyPaths: [...new Set(parsed.dirtyPaths.filter((path) => filePaths.has(path)))],
      explorerWidth: Math.min(420, Math.max(180, parsed.explorerWidth)),
      bottomHeight: parsed.bottomHeight === 0 ? 0 : Math.min(480, Math.max(120, parsed.bottomHeight)),
    };
  } catch {
    return structuredClone(DEFAULT_SNAPSHOT);
  }
}

export function persistSnapshot(storage: StorageLike, snapshot: WorkspaceSnapshot, key = STORAGE_KEY): void {
  storage.setItem(key, JSON.stringify(snapshot));
}
