import { StreamLanguage, type StreamParser } from '@codemirror/language';

const KEYWORDS = new Set(['LET', 'SET', 'SAY', 'TO', 'GIVES', 'AS', 'IF', 'THEN', 'ELSE', 'END', 'REPEAT', 'FROM', 'WHILE', 'FOR', 'EACH', 'IN', 'CLASS', 'EXTENDS', 'NEW', 'RETURN', 'TRY', 'CATCH', 'USE', 'TRUE', 'FALSE', 'AND', 'OR', 'NOT']);
const TYPES = new Set(['INT', 'INTEGER', 'FLOAT', 'NUMBER', 'STRING', 'BOOL', 'BOOLEAN', 'ANY', 'VOID']);

export function classifyPlainWord(word: string): 'keyword' | 'typeName' | 'variableName' {
  const upper = word.toUpperCase();
  if (KEYWORDS.has(upper)) return 'keyword';
  if (TYPES.has(upper)) return 'typeName';
  return 'variableName';
}

const parser: StreamParser<null> = {
  startState: () => null,
  token(stream) {
    if (stream.eatSpace()) return null;
    if (stream.match(/#[^\n]*/)) return 'comment';
    if (stream.match(/"(?:\\.|[^"\\])*"/)) return 'string';
    if (stream.match(/'(?:\\.|[^'\\])*'/)) return 'string';
    if (stream.match(/\d+(?:\.\d+)?/)) return 'number';
    if (stream.match(/(?:<=|>=|!=|==|<>|[=<>+\-*\/%])/)) return 'operator';
    if (stream.match(/[A-Za-z_][A-Za-z0-9_]*/)) {
      const word = stream.current();
      return classifyPlainWord(word);
    }
    stream.next();
    return null;
  },
};

export const plainLanguage = StreamLanguage.define(parser);
