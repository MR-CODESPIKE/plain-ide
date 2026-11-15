import { AlertTriangle, CheckCircle2, CircleHelp, TerminalSquare } from 'lucide-react';

export interface OutputState {
  title: string;
  lines: string[];
  kind: 'idle' | 'success' | 'error' | 'info';
  timestamp?: string;
}

interface Props { output: OutputState }

export function OutputPanel({ output }: Props) {
  const Icon = output.kind === 'error' ? AlertTriangle : output.kind === 'success' ? CheckCircle2 : output.kind === 'info' ? CircleHelp : TerminalSquare;
  return (
    <div className="output-panel" aria-live="polite">
      <div className="output-meta"><Icon size={15} className={`output-icon ${output.kind}`} /><span>{output.title}</span>{output.timestamp && <time>{output.timestamp}</time>}</div>
      {output.lines.length ? <pre className={output.kind === 'error' ? 'output-text error-text' : 'output-text'}>{output.lines.join('\n')}</pre> : <div className="output-empty"><span className="output-prompt">&gt;_</span><p>Run a Plain file or check its syntax to see output here.</p></div>}
    </div>
  );
}
