import { useEffect, useRef, useState, type FormEvent } from 'react';
import { ChevronDown, CornerDownLeft } from 'lucide-react';
import type { WorkspaceEntry } from '../types';
import { executeTerminalCommand } from '../lib/terminal';
import type { RunResult } from '../types';

type TerminalLine = { prompt?: string; text: string; kind: 'command' | 'output' | 'error' | 'muted' };
interface Props { entries: WorkspaceEntry[]; runPath: (path: string) => Promise<RunResult> }

export function TerminalPanel({ entries, runPath }: Props) {
  const [value, setValue] = useState('');
  const [lines, setLines] = useState<TerminalLine[]>([{ text: 'Plain workspace terminal · type help for available commands.', kind: 'muted' }]);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const end = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { end.current?.scrollIntoView({ block: 'end' }); }, [lines]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const command = value.trim();
    if (!command) return;
    setValue('');
    setHistory((current) => [...current, command]);
    setHistoryIndex(-1);
    const result = executeTerminalCommand(command, entries);
    if (result.clear) { setLines([]); return; }
    setLines((current) => [...current, { prompt: command, text: '', kind: 'command' }, ...(result.text ? [{ text: result.text, kind: 'output' as const }] : [])]);
    if (result.runPath) {
      const run = await runPath(result.runPath);
      setLines((current) => [...current,
        ...run.output.map((text) => ({ text, kind: 'output' as const })),
        ...(run.ok ? [{ text: `Process finished · ${run.steps} statements`, kind: 'muted' as const }] : [{ text: run.error ?? 'Run failed', kind: 'error' as const }]),
      ]);
    }
  }

  function handleHistory(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      const next = Math.max(0, historyIndex < 0 ? history.length - 1 : historyIndex - 1);
      setHistoryIndex(next);
      setValue(history[next] ?? '');
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      const next = historyIndex + 1;
      if (next >= history.length) { setHistoryIndex(-1); setValue(''); }
      else { setHistoryIndex(next); setValue(history[next] ?? ''); }
    }
  }

  return (
    <section className="terminal-panel" aria-label="Workspace terminal">
      <div className="terminal-toolbar"><span className="terminal-cwd"><span className="terminal-live" /> /workspace</span><button className="terminal-shell-select" type="button" title="Safe workspace command runner" aria-label="Terminal type: workspace commands">workspace commands <ChevronDown size={13} /></button></div>
      <div className="terminal-scroll" onClick={() => input.current?.focus()}>
        {lines.map((line, index) => (
          <div className={`terminal-line ${line.kind}`} key={`${index}-${line.prompt ?? line.text}`}>
            {line.prompt && <><span className="terminal-chevron">›</span><span className="terminal-command">{line.prompt}</span></>}
            {line.text && <pre>{line.text}</pre>}
          </div>
        ))}
        <div ref={end} />
      </div>
      <form className="terminal-input-row" onSubmit={submit}>
        <span className="terminal-chevron">›</span>
        <input ref={input} aria-label="Enter workspace command" autoComplete="off" spellCheck={false} value={value} onChange={(event) => setValue(event.target.value)} onKeyDown={handleHistory} placeholder="Type help to see supported commands" />
        <button className="terminal-submit" type="submit" aria-label="Run terminal command"><CornerDownLeft size={15} /></button>
      </form>
    </section>
  );
}
