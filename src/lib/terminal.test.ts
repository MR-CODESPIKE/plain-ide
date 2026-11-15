import { describe, expect, it } from 'vitest';
import { executeTerminalCommand } from './terminal';
import { DEFAULT_WORKSPACE } from './defaultWorkspace';

describe('safe workspace terminal', () => {
  it('shows supported commands and virtual working directory', () => {
    expect(executeTerminalCommand('pwd', DEFAULT_WORKSPACE).text).toBe('/workspace');
    expect(executeTerminalCommand('help', DEFAULT_WORKSPACE).text).toContain('plain run');
  });

  it('lists and reads files inside the workspace', () => {
    expect(executeTerminalCommand('ls src', DEFAULT_WORKSPACE).text).toContain('src/main.plain');
    expect(executeTerminalCommand('cat src/main.plain', DEFAULT_WORKSPACE).text).toContain('REPEAT i');
  });

  it('dispatches Plain syntax check and run only for workspace source files', () => {
    expect(executeTerminalCommand('plain check src/main.plain', DEFAULT_WORKSPACE).text).toContain('Syntax is valid');
    expect(executeTerminalCommand('plain run src/main.plain', DEFAULT_WORKSPACE).runPath).toBe('src/main.plain');
    expect(executeTerminalCommand('plain run missing.plain', DEFAULT_WORKSPACE).text).toContain('File not found');
  });

  it('does not execute arbitrary host shell commands', () => {
    expect(executeTerminalCommand('rm -rf /', DEFAULT_WORKSPACE).text).toContain('Unknown workspace command');
  });

  it('finds text across project files and supports clearing output', () => {
    expect(executeTerminalCommand('find sum', DEFAULT_WORKSPACE).text).toContain('src/main.plain');
    expect(executeTerminalCommand('clear', DEFAULT_WORKSPACE).clear).toBe(true);
  });
});
