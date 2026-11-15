import { describe, expect, it } from 'vitest';
import { classifyPlainWord } from './plainLanguage';

describe('Plain editor token categories', () => {
  it('recognizes language keywords without case sensitivity', () => {
    expect(classifyPlainWord('LET')).toBe('keyword');
    expect(classifyPlainWord('repeat')).toBe('keyword');
    expect(classifyPlainWord('Say')).toBe('keyword');
  });

  it('recognizes common type names and ordinary identifiers', () => {
    expect(classifyPlainWord('INTEGER')).toBe('typeName');
    expect(classifyPlainWord('String')).toBe('typeName');
    expect(classifyPlainWord('runningTotal')).toBe('variableName');
  });
});
