import type { RunResult } from '../types';

type Value = number | string | boolean;
type Token = { kind: 'number' | 'string' | 'word' | 'operator'; value: string };
type Statement =
  | { kind: 'let'; name: string; expression: string; line: number }
  | { kind: 'set'; name: string; expression: string; line: number }
  | { kind: 'say'; expression: string; line: number }
  | { kind: 'repeat'; name: string; start: string; end: string; body: Statement[]; line: number }
  | { kind: 'if'; expression: string; yes: Statement[]; no: Statement[]; line: number };

const MAX_STEPS = 100_000;
const MAX_LOOP_RANGE = 50_000;
const MAX_SOURCE_LENGTH = 100_000;
const MAX_OUTPUT_LINES = 1_000;

export class PlainSyntaxError extends Error {
  constructor(message: string, readonly line: number) {
    super(`${message} (line ${line})`);
    this.name = 'PlainSyntaxError';
  }
}

function tokenize(source: string, line: number): Token[] {
  const tokens: Token[] = [];
  let rest = source;
  while (rest.length) {
    const space = rest.match(/^\s+/);
    if (space) { rest = rest.slice(space[0].length); continue; }
    const match = rest.match(/^(?:(\d+(?:\.\d+)?)|("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')|([A-Za-z_][A-Za-z0-9_]*)|(<=|>=|!=|==|<>|[()+\-*\/%=< >]))/);
    if (!match) throw new PlainSyntaxError(`Unexpected character near “${rest.slice(0, 12)}”`, line);
    const raw = match[0];
    if (match[1]) tokens.push({ kind: 'number', value: match[1] });
    else if (match[2]) tokens.push({ kind: 'string', value: match[2] });
    else if (match[3]) tokens.push({ kind: 'word', value: match[3] });
    else tokens.push({ kind: 'operator', value: raw.trim() });
    rest = rest.slice(raw.length);
  }
  return tokens;
}

const PRECEDENCE: Record<string, number> = { OR: 1, AND: 2, '=': 3, '==': 3, '!=': 3, '<>': 3, '<': 3, '>': 3, '<=': 3, '>=': 3, '+': 4, '-': 4, '*': 5, '/': 5, '%': 5 };

class ExpressionParser {
  private cursor = 0;
  constructor(private readonly tokens: Token[], private readonly variables: Map<string, Value>, private readonly line: number) {}

  parse(): Value {
    const value = this.expression(0);
    if (this.cursor !== this.tokens.length) throw new PlainSyntaxError(`Unexpected token “${this.tokens[this.cursor].value}”`, this.line);
    return value;
  }

  private expression(minimum: number): Value {
    let left = this.unary();
    while (this.cursor < this.tokens.length) {
      const token = this.tokens[this.cursor];
      const operator = token.value.toUpperCase();
      const precedence = token.kind === 'operator' || token.kind === 'word' ? PRECEDENCE[operator] : undefined;
      if (precedence === undefined || precedence < minimum) break;
      this.cursor += 1;
      const right = this.expression(precedence + 1);
      left = this.apply(operator, left, right);
    }
    return left;
  }

  private unary(): Value {
    const token = this.tokens[this.cursor];
    if (!token) throw new PlainSyntaxError('Expected an expression', this.line);
    const operator = token.value.toUpperCase();
    if (operator === '-' || operator === '+' || operator === 'NOT') {
      this.cursor += 1;
      const value = this.unary();
      if (operator === 'NOT') return !truthy(value);
      return operator === '-' ? -numeric(value, this.line) : numeric(value, this.line);
    }
    if (token.value === '(') {
      this.cursor += 1;
      const value = this.expression(0);
      if (this.tokens[this.cursor]?.value !== ')') throw new PlainSyntaxError('Expected )', this.line);
      this.cursor += 1;
      return value;
    }
    this.cursor += 1;
    if (token.kind === 'number') return Number(token.value);
    if (token.kind === 'string') {
      const inner = token.value.slice(1, -1);
      return inner.replace(/\\n/g, '\n').replace(/\\t/g, '\t').replace(/\\([\\"'])/g, '$1');
    }
    if (token.kind === 'word') {
      const word = token.value.toUpperCase();
      if (word === 'TRUE') return true;
      if (word === 'FALSE') return false;
      const name = token.value.toLocaleLowerCase();
      if (!this.variables.has(name)) throw new PlainSyntaxError(`Unknown variable “${token.value}”`, this.line);
      return this.variables.get(name)!;
    }
    throw new PlainSyntaxError(`Expected a value, got “${token.value}”`, this.line);
  }

  private apply(operator: string, left: Value, right: Value): Value {
    if (operator === 'AND') return truthy(left) && truthy(right);
    if (operator === 'OR') return truthy(left) || truthy(right);
    if (operator === '=' || operator === '==') return left === right || (typeof left !== 'string' && typeof right !== 'string' && Number(left) === Number(right));
    if (operator === '!=' || operator === '<>') return !(left === right || (typeof left !== 'string' && typeof right !== 'string' && Number(left) === Number(right)));
    if (operator === '+' && (typeof left === 'string' || typeof right === 'string')) return display(left) + display(right);
    if (operator === '<' || operator === '>' || operator === '<=' || operator === '>=') {
      if (typeof left === 'string' && typeof right === 'string') {
        if (operator === '<') return left < right;
        if (operator === '>') return left > right;
        if (operator === '<=') return left <= right;
        return left >= right;
      }
      const a = numeric(left, this.line); const b = numeric(right, this.line);
      if (operator === '<') return a < b;
      if (operator === '>') return a > b;
      if (operator === '<=') return a <= b;
      return a >= b;
    }
    const a = numeric(left, this.line); const b = numeric(right, this.line);
    if (operator === '+') return a + b;
    if (operator === '-') return a - b;
    if (operator === '*') return a * b;
    if (operator === '/') {
      if (b === 0) throw new PlainSyntaxError('Division by zero', this.line);
      return a / b;
    }
    if (operator === '%') {
      if (b === 0) throw new PlainSyntaxError('Modulo by zero', this.line);
      return a % b;
    }
    throw new PlainSyntaxError(`Unsupported operator “${operator}”`, this.line);
  }
}

function numeric(value: Value, line: number): number {
  const parsed = typeof value === 'number' ? value : typeof value === 'boolean' ? (value ? 1 : 0) : Number(value);
  if (!Number.isFinite(parsed)) throw new PlainSyntaxError(`Expected a finite number, got “${value}”`, line);
  return parsed;
}

function truthy(value: Value): boolean {
  return typeof value === 'string' ? value.length > 0 : Boolean(value);
}

function display(value: Value): string {
  if (typeof value === 'boolean') return value ? 'TRUE' : 'FALSE';
  if (typeof value === 'number' && Number.isInteger(value)) return String(value);
  return String(value);
}

function evaluate(expression: string, variables: Map<string, Value>, line: number): Value {
  return new ExpressionParser(tokenize(expression, line), variables, line).parse();
}

function stripComment(line: string): string {
  let quote = '';
  let escaped = false;
  for (let i = 0; i < line.length; i += 1) {
    const char = line[i];
    if (escaped) { escaped = false; continue; }
    if (char === '\\' && quote) { escaped = true; continue; }
    if ((char === '"' || char === "'") && (!quote || quote === char)) { quote = quote ? '' : char; continue; }
    if (char === '#' && !quote) return line.slice(0, i);
  }
  return line;
}

interface ParsedBlock { statements: Statement[]; cursor: number; stop: string }

function parseBlock(lines: string[], start: number, stops: string[]): ParsedBlock {
  const statements: Statement[] = [];
  let cursor = start;
  while (cursor < lines.length) {
    const raw = stripComment(lines[cursor]).trim();
    const normalized = raw.toUpperCase().replace(/\s+/g, ' ');
    if (stops.includes(normalized)) return { statements, cursor, stop: normalized };
    if (!raw) { cursor += 1; continue; }
    const line = cursor + 1;
    const assignment = raw.match(/^(LET|SET)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$/i);
    if (assignment) {
      const kind = assignment[1].toLocaleLowerCase() === 'let' ? 'let' : 'set';
      statements.push({ kind, name: assignment[2], expression: assignment[3], line });
      cursor += 1;
      continue;
    }
    const say = raw.match(/^SAY\s+(.+)$/i);
    if (say) {
      statements.push({ kind: 'say', expression: say[1], line });
      cursor += 1;
      continue;
    }
    const repeat = raw.match(/^REPEAT\s+([A-Za-z_][A-Za-z0-9_]*)\s+FROM\s+(.+?)\s+TO\s+(.+)$/i);
    if (repeat) {
      const body = parseBlock(lines, cursor + 1, ['END REPEAT']);
      if (body.stop !== 'END REPEAT') throw new PlainSyntaxError('REPEAT is missing END REPEAT', line);
      statements.push({ kind: 'repeat', name: repeat[1], start: repeat[2], end: repeat[3], body: body.statements, line });
      cursor = body.cursor + 1;
      continue;
    }
    const condition = raw.match(/^IF\s+(.+)\s+THEN\s*$/i);
    if (condition) {
      const yes = parseBlock(lines, cursor + 1, ['ELSE', 'END IF']);
      let no: Statement[] = [];
      let endCursor = yes.cursor;
      if (yes.stop === 'ELSE') {
        const otherwise = parseBlock(lines, yes.cursor + 1, ['END IF']);
        if (otherwise.stop !== 'END IF') throw new PlainSyntaxError('IF is missing END IF', line);
        no = otherwise.statements;
        endCursor = otherwise.cursor;
      } else if (yes.stop !== 'END IF') {
        throw new PlainSyntaxError('IF is missing END IF', line);
      }
      statements.push({ kind: 'if', expression: condition[1], yes: yes.statements, no, line });
      cursor = endCursor + 1;
      continue;
    }
    if (/^(END\b|ELSE\b)/i.test(raw)) throw new PlainSyntaxError(`Unexpected block terminator “${raw}”`, line);
    throw new PlainSyntaxError('Unsupported statement; preview supports LET, SET, SAY, IF/ELSE, and counted REPEAT', line);
  }
  return { statements, cursor, stop: '' };
}

function parseProgram(source: string): Statement[] {
  if (source.length > MAX_SOURCE_LENGTH) throw new PlainSyntaxError('Source is too large for the browser preview (100 KB limit)', 1);
  const lines = source.replace(/\r\n/g, '\n').replace(/\r/g, '\n').split('\n');
  const block = parseBlock(lines, 0, []);
  return block.statements;
}

export function checkPlain(source: string): { ok: boolean; message: string } {
  try {
    parseProgram(source);
    return { ok: true, message: 'Syntax is valid for the Plain IDE preview subset.' };
  } catch (error) {
    return { ok: false, message: error instanceof Error ? error.message : String(error) };
  }
}

export function runPlain(source: string): RunResult {
  const output: string[] = [];
  const variables = new Map<string, Value>();
  let steps = 0;
  const tick = (line: number) => {
    steps += 1;
    if (steps > MAX_STEPS) throw new PlainSyntaxError(`Execution limit exceeded (${MAX_STEPS} statements)`, line);
  };
  const execute = (statements: Statement[]): void => {
    for (const statement of statements) {
      tick(statement.line);
      if (statement.kind === 'let') {
        const name = statement.name.toLocaleLowerCase();
        if (variables.has(name)) throw new PlainSyntaxError(`Variable “${statement.name}” is already declared`, statement.line);
        variables.set(name, evaluate(statement.expression, variables, statement.line));
      } else if (statement.kind === 'set') {
        const name = statement.name.toLocaleLowerCase();
        if (!variables.has(name)) throw new PlainSyntaxError(`Cannot SET unknown variable “${statement.name}”`, statement.line);
        variables.set(name, evaluate(statement.expression, variables, statement.line));
      } else if (statement.kind === 'say') {
        if (output.length >= MAX_OUTPUT_LINES) throw new PlainSyntaxError('Output limit exceeded (1,000 lines)', statement.line);
        output.push(display(evaluate(statement.expression, variables, statement.line)));
      } else if (statement.kind === 'if') {
        execute(truthy(evaluate(statement.expression, variables, statement.line)) ? statement.yes : statement.no);
      } else {
        const start = Math.ceil(numeric(evaluate(statement.start, variables, statement.line), statement.line));
        const end = Math.floor(numeric(evaluate(statement.end, variables, statement.line), statement.line));
        if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end)) {
          throw new PlainSyntaxError('REPEAT bounds must resolve to safe whole numbers', statement.line);
        }
        const count = Math.max(0, end - start + 1);
        if (!Number.isSafeInteger(count) || count > MAX_LOOP_RANGE) {
          throw new PlainSyntaxError(`Loop range exceeds the preview limit (${MAX_LOOP_RANGE})`, statement.line);
        }
        const name = statement.name.toLocaleLowerCase();
        for (let index = 0; index < count; index += 1) {
          const value = start + index;
          if (!Number.isSafeInteger(value)) throw new PlainSyntaxError('REPEAT counter exceeded the safe integer range', statement.line);
          variables.set(name, value);
          execute(statement.body);
        }
      }
    }
  };
  try {
    execute(parseProgram(source));
    return { ok: true, output, steps };
  } catch (error) {
    return { ok: false, output, error: error instanceof Error ? error.message : String(error), steps };
  }
}
