import type { WorkspaceEntry, WorkspaceSnapshot } from '../types';

export const DEFAULT_WORKSPACE: WorkspaceEntry[] = [
  { path: 'src', kind: 'folder', content: '' },
  {
    path: 'src/main.plain',
    kind: 'file',
    content: '# Plain IDE preview runner: LET, SET, REPEAT, IF, and SAY are supported.\nLET limit = 8\nLET total = 0\nREPEAT i FROM 1 TO limit\n  SET total = total + i\nEND REPEAT\nSAY "sum 1.." + limit + " = " + total\n',
  },
  { path: 'examples', kind: 'folder', content: '' },
  {
    path: 'examples/loops.plain',
    kind: 'file',
    content: 'LET evenTotal = 0\nREPEAT n FROM 1 TO 10\n  IF n % 2 = 0 THEN\n    SET evenTotal = evenTotal + n\n  END IF\nEND REPEAT\nSAY "even total = " + evenTotal\n',
  },
  {
    path: 'README.md',
    kind: 'file',
    content: '# Plain IDE workspace\n\nEdit `.plain` files and run them with the built-in preview interpreter. The preview supports a safe subset of Plain: LET, SET, SAY, IF/ELSE, counted REPEAT loops, arithmetic, comparisons, strings, and booleans. Use the full Plain compiler for complete language support.\n',
  },
];

export const DEFAULT_SNAPSHOT: WorkspaceSnapshot = {
  entries: DEFAULT_WORKSPACE,
  openPaths: ['src/main.plain', 'README.md'],
  activePath: 'src/main.plain',
  dirtyPaths: [],
  explorerWidth: 248,
  bottomHeight: 238,
  bottomTab: 'output',
};
