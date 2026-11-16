import { describe, expect, it } from 'vitest';
import { createFile, createFolder, deleteEntry, renameEntry, searchWorkspace, updateFile } from './workspace';
import type { WorkspaceEntry } from '../types';

const seed: WorkspaceEntry[] = [
  { path: 'src', kind: 'folder', content: '' },
  { path: 'src/main.plain', kind: 'file', content: 'SAY "hello"' },
  { path: 'src/lib', kind: 'folder', content: '' },
  { path: 'src/lib/math.plain', kind: 'file', content: 'LET total = 42' },
  { path: 'README.md', kind: 'file', content: 'hello project' },
];

describe('workspace tree operations', () => {
  it('creates nested files and missing parent folders', () => {
    const next = createFile([], 'app/models/user.plain');
    expect(next.map((entry) => entry.path)).toEqual(['app', 'app/models', 'app/models/user.plain']);
  });

  it('updates file content without changing other entries', () => {
    const next = updateFile(seed, 'src/main.plain', 'SAY "updated"');
    expect(next.find((entry) => entry.path === 'src/main.plain')?.content).toBe('SAY "updated"');
    expect(next.find((entry) => entry.path === 'README.md')?.content).toBe('hello project');
  });

  it('renames a folder and every descendant once', () => {
    const next = renameEntry(seed, 'src', 'source');
    expect(next.map((entry) => entry.path).sort()).toEqual(['README.md', 'source', 'source/lib', 'source/lib/math.plain', 'source/main.plain']);
    expect(next).toHaveLength(seed.length);
  });

  it('rejects collisions and unsafe names', () => {
    const withCollision = [...seed, { path: 'src/other.plain', kind: 'file' as const, content: '' }];
    expect(() => renameEntry(withCollision, 'src/main.plain', 'other.plain')).toThrow(/overwrite/i);
    expect(() => createFolder(seed, '../outside')).toThrow(/project-relative/i);
    expect(() => renameEntry(seed, 'src', '../outside')).toThrow(/valid name/i);
  });

  it('searches paths and source content case-insensitively', () => {
    expect(searchWorkspace(seed, 'HELLO').map((entry) => entry.path).sort()).toEqual(['README.md', 'src/main.plain']);
    expect(searchWorkspace(seed, 'main')).toHaveLength(1);
    expect(searchWorkspace(seed, '')).toEqual([]);
  });

  it('deletes a folder and all nested entries', () => {
    expect(deleteEntry(seed, 'src')).toEqual([seed[4]]);
  });

  it('prevents files from becoming parents of other workspace entries', () => {
    expect(() => createFile(seed, 'src/main.plain/child.plain')).toThrow(/is a file/i);
    expect(() => createFile(seed, 'src')).toThrow(/already exists/i);
    expect(() => createFile([{ path: 'docs/guide.md', kind: 'file', content: '' }], 'docs')).toThrow(/already contains entries/i);
  });

  it('rejects rename operations that create file/folder prefix conflicts', () => {
    const entries = [
      { path: 'a.plain', kind: 'file' as const, content: '' },
      { path: 'docs/readme.md', kind: 'file' as const, content: '' },
    ];
    expect(() => renameEntry(entries, 'a.plain', 'docs')).toThrow(/conflict|overwrite/i);
  });
});
