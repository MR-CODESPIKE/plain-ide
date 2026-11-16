import test from 'node:test';
import assert from 'node:assert/strict';
import { safeRelativePath } from './security.mjs';

test('accepts normal workspace paths', () => {
  assert.equal(safeRelativePath('main.plain'), true);
  assert.equal(safeRelativePath('src/helpers.plain'), true);
});

test('rejects traversal and absolute paths', () => {
  for (const path of ['../secret', 'src/../../secret', '/etc/passwd', 'C:\\secret', 'src\\main.plain', '', '.', 'src/./main.plain', 'src//main.plain', 'folder/file:stream.plain', 'folder/bad?.plain']) {
    assert.equal(safeRelativePath(path), false, path);
  }
});

test('rejects non-string paths and null bytes', () => {
  assert.equal(safeRelativePath(null), false);
  assert.equal(safeRelativePath('../bad\0.plain'), false);
});
