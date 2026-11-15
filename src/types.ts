export type EntryKind = 'file' | 'folder';

export interface WorkspaceEntry {
  path: string;
  kind: EntryKind;
  content: string;
}

export interface WorkspaceSnapshot {
  entries: WorkspaceEntry[];
  openPaths: string[];
  activePath: string;
  dirtyPaths: string[];
  explorerWidth: number;
  bottomHeight: number;
  bottomTab: 'output' | 'terminal';
}

export interface RunResult {
  ok: boolean;
  output: string[];
  error?: string;
  steps: number;
}
