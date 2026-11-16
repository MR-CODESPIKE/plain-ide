import path from 'node:path';

export function safeRelativePath(input) {
  if (typeof input !== 'string' || !input || /[\0-\x1f<>:"|?*\\]/.test(input)) return false;
  if (path.posix.isAbsolute(input) || /^[a-zA-Z]:/.test(input)) return false;
  const parts = input.split('/');
  return parts.every(part => part && part !== '.' && part !== '..');
}
