export function replaceAllText(text: string, search: string, replacement: string, caseSensitive = false): { text: string; count: number } {
  if (!search) return { text, count: 0 };
  const source = caseSensitive ? text : text.toLocaleLowerCase();
  const needle = caseSensitive ? search : search.toLocaleLowerCase();
  let cursor = 0;
  let count = 0;
  let result = '';
  while (true) {
    const at = source.indexOf(needle, cursor);
    if (at < 0) break;
    result += text.slice(cursor, at) + replacement;
    cursor = at + search.length;
    count += 1;
  }
  return { text: result + text.slice(cursor), count };
}
