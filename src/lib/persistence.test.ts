import { describe, expect, it } from 'vitest';
import { DEFAULT_SNAPSHOT } from './defaultWorkspace';
import { persistSnapshot, restoreSnapshot } from './persistence';
import type { WorkspaceSnapshot } from '../types';

class MemoryStorage {
  private readonly values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

describe('local workspace persistence', () => {
  it('restores edited drafts, open tabs, active file, and dirty indicators', () => {
    const storage = new MemoryStorage();
    const changed: WorkspaceSnapshot = {
      ...structuredClone(DEFAULT_SNAPSHOT),
      entries: DEFAULT_SNAPSHOT.entries.map((entry) => entry.path === 'src/main.plain' ? { ...entry, content: 'SAY "draft"' } : entry),
      dirtyPaths: ['src/main.plain'],
    };
    persistSnapshot(storage, changed);
    const restored = restoreSnapshot(storage);
    expect(restored.entries.find((entry) => entry.path === 'src/main.plain')?.content).toBe('SAY "draft"');
    expect(restored.activePath).toBe('src/main.plain');
    expect(restored.dirtyPaths).toContain('src/main.plain');
  });

  it('drops stale tabs and clamps panel sizes to safe bounds', () => {
    const storage = new MemoryStorage();
    const changed = { ...structuredClone(DEFAULT_SNAPSHOT), openPaths: ['missing.plain'], activePath: 'missing.plain', explorerWidth: 999, bottomHeight: 1 };
    persistSnapshot(storage, changed);
    const restored = restoreSnapshot(storage);
    expect(restored.openPaths).toEqual([]);
    expect(restored.activePath).toBe('');
    expect(restored.explorerWidth).toBe(420);
    expect(restored.bottomHeight).toBe(120);
  });

  it('uses the starter workspace for malformed or inaccessible stored data', () => {
    const storage = new MemoryStorage();
    storage.setItem('plain-ide.workspace.v1', '{not json');
    expect(restoreSnapshot(storage).activePath).toBe(DEFAULT_SNAPSHOT.activePath);
    expect(restoreSnapshot({ getItem: () => { throw new Error('blocked'); }, setItem: () => undefined }).entries).toHaveLength(DEFAULT_SNAPSHOT.entries.length);
  });
});
