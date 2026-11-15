import { describe, expect, it } from 'vitest';
import { replaceAllText } from './textSearch';

describe('in-file find and replace', () => {
  it('replaces every matching occurrence and returns a count', () => {
    expect(replaceAllText('foo = foo + 1', 'foo', 'total')).toEqual({ text: 'total = total + 1', count: 2 });
  });

  it('supports case-insensitive replacement and empty-query no-op', () => {
    expect(replaceAllText('Say SAY', 'say', 'print').text).toBe('print print');
    expect(replaceAllText('unchanged', '', 'x')).toEqual({ text: 'unchanged', count: 0 });
  });
});
