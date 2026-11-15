import type { WorkspaceEntry } from '../types';
import { findFile, searchWorkspace } from './workspace';
import { checkPlain } from './plainRuntime';

export interface TerminalResult { text: string; clear?: boolean; runPath?: string }

function tokenizeCommand(input: string): string[] {
  const parts = input.match(/(?:[^\s"']+|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')+/g) ?? [];
  return parts.map((part) => part.length >= 2 && (part[0] === '"' || part[0] === "'") ? part.slice(1, -1) : part);
}

export function executeTerminalCommand(input: string, entries: WorkspaceEntry[]): TerminalResult {
  const args = tokenizeCommand(input.trim());
  const command = (args.shift() ?? '').toLocaleLowerCase();
  if (!command) return { text: '' };
  if (command === 'clear') return { text: '', clear: true };
  if (command === 'help') return { text: 'Workspace terminal commands:\n  help                 Show this list\n  pwd                  Show the virtual workspace path\n  ls [folder]          List files and folders\n  cat <file>           Print a file\n  find <text>          Search file names and contents\n  plain check <file>   Check supported Plain preview syntax\n  plain run <file>     Run a .plain file in the safe preview interpreter\n  clear                Clear terminal output\n\nThis is a workspace-only command runner, not an operating-system shell.' };
  if (command === 'pwd') return { text: '/workspace' };
  if (command === 'ls' || command === 'tree') {
    const folder = args[0] ?? '';
    const normalizedFolder = folder.replace(/^\/+|\/+$/g, '');
    const children = entries.filter((entry) => {
      const parent = entry.path.includes('/') ? entry.path.slice(0, entry.path.lastIndexOf('/')) : '';
      return parent === normalizedFolder;
    });
    return { text: children.length ? children.sort((a, b) => a.kind === b.kind ? a.path.localeCompare(b.path) : a.kind === 'folder' ? -1 : 1).map((entry) => `${entry.kind === 'folder' ? '▸' : '·'} ${entry.path}`).join('\n') : `No entries in ${normalizedFolder || '/workspace'}.` };
  }
  if (command === 'cat') {
    const path = args[0];
    if (!path) return { text: 'Usage: cat <file>' };
    const file = findFile(entries, path);
    return file ? { text: file.content || '(empty file)' } : { text: `File not found: ${path}` };
  }
  if (command === 'find' || command === 'grep') {
    const query = args.join(' ');
    if (!query) return { text: 'Usage: find <text>' };
    const matches = searchWorkspace(entries, query);
    return { text: matches.length ? matches.map((entry) => {
      const line = entry.content.split(/\r?\n/).findIndex((value) => value.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
      return `${entry.path}${line >= 0 ? `:${line + 1}` : ''}`;
    }).join('\n') : `No matches for “${query}”.` };
  }
  if (command === 'plain') {
    const action = (args.shift() ?? '').toLocaleLowerCase();
    const path = args[0];
    if (!['run', 'check'].includes(action) || !path) return { text: 'Usage: plain <run|check> <file.plain>' };
    const file = findFile(entries, path);
    if (!file) return { text: `File not found: ${path}` };
    if (!path.endsWith('.plain')) return { text: 'The preview runner accepts .plain source files only.' };
    const checked = checkPlain(file.content);
    if (!checked.ok) return { text: checked.message };
    return action === 'run' ? { text: '', runPath: path } : { text: checked.message };
  }
  return { text: `Unknown workspace command: ${command}. Type help for supported commands.` };
}
