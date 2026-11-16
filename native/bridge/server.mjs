#!/usr/bin/env node
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs/promises';
import { randomBytes, timingSafeEqual } from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { safeRelativePath } from './security.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', 'plain-ecosystem');
const IS_WIN = process.platform === 'win32';
const DEFAULT_COMPILER = path.join(REPO, 'target', 'release', IS_WIN ? 'plain.exe' : 'plain');
const COMPILER = process.env.PLAIN_COMPILER_PATH || DEFAULT_COMPILER;
const PORT = Number(process.env.PLAIN_IDE_PORT || 4317);
const HOST = '127.0.0.1';
const TOKEN = process.env.PLAIN_IDE_TOKEN || randomBytes(24).toString('hex');
const MAX_BODY = 2 * 1024 * 1024;
const MAX_FILES = 300;
const MAX_OUTPUT = 1024 * 1024;
const TIMEOUT_MS = 45_000;

function send(res, status, payload, origin = '') {
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
    'access-control-allow-origin': origin || 'null',
    'vary': 'Origin',
    'x-content-type-options': 'nosniff',
  });
  res.end(JSON.stringify(payload));
}

function allowedOrigin(origin) {
  if (!origin) return true;
  if (/^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/.test(origin)) return true;
  const extra = (process.env.PLAIN_IDE_ORIGINS || '').split(',').map(x => x.trim()).filter(Boolean);
  return extra.includes(origin);
}

function readJson(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.setEncoding('utf8');
    req.on('data', chunk => {
      body += chunk;
      if (Buffer.byteLength(body) > MAX_BODY) {
        reject(Object.assign(new Error('Request body exceeds 2 MiB.'), { status: 413 }));
        req.destroy();
      }
    });
    req.on('end', () => {
      try { resolve(JSON.parse(body || '{}')); }
      catch { reject(Object.assign(new Error('Request must contain valid JSON.'), { status: 400 })); }
    });
    req.on('error', reject);
  });
}

function runCompiler(args, cwd, stdin = '', extraEnv = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(COMPILER, args, {
      cwd,
      windowsHide: true,
      env: {
        ...process.env,
        CARGO_NET_OFFLINE: 'true',
        PLAIN_ALLOW_EXTERNAL_IMPORTS: '0',
        ...extraEnv,
      },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdin.end(stdin);
    let settled = false;
    const timer = setTimeout(() => {
      child.kill('SIGKILL');
      if (!settled) { settled = true; reject(new Error('Compiler exceeded the 45-second execution limit.')); }
    }, TIMEOUT_MS);
    const append = (current, chunk) => (current + chunk.toString()).slice(-MAX_OUTPUT);
    child.stdout.on('data', chunk => { stdout = append(stdout, chunk); });
    child.stderr.on('data', chunk => { stderr = append(stderr, chunk); });
    child.on('error', error => {
      clearTimeout(timer);
      if (!settled) { settled = true; reject(new Error(`Could not start Plain compiler: ${error.message}`)); }
    });
    child.on('close', (code, signal) => {
      clearTimeout(timer);
      if (!settled) {
        settled = true;
        resolve({ code: code ?? (signal ? 1 : 0), stdout, stderr });
      }
    });
  });
}

const server = http.createServer(async (req, res) => {
  const origin = req.headers.origin || '';
  if (!allowedOrigin(origin)) return send(res, 403, { error: 'Origin is not allowed. Set PLAIN_IDE_ORIGINS to permit this IDE origin.' }, 'null');
  const corsOrigin = origin || 'null';
  res.setHeader('access-control-allow-methods', 'GET, POST, OPTIONS');
  res.setHeader('access-control-allow-headers', 'content-type, authorization');
  if (req.method === 'OPTIONS') { res.writeHead(204, { 'access-control-allow-origin': corsOrigin, 'access-control-allow-methods': 'GET, POST, OPTIONS', 'access-control-allow-headers': 'content-type, authorization', 'access-control-allow-private-network': 'true', 'vary': 'Origin' }); return res.end(); }

  const url = new URL(req.url || '/', `http://${HOST}:${PORT}`);
  if (req.method === 'GET' && url.pathname === '/api/health') {
    return send(res, 200, { service: 'plain-ide-native-bridge', version: 1, compilerInstalled: await fs.access(COMPILER).then(() => true, () => false), platform: process.platform, arch: process.arch }, corsOrigin);
  }
  const supplied = (req.headers.authorization || '').replace(/^Bearer\s+/i, '');
  const expected = Buffer.from(TOKEN);
  const actual = Buffer.from(supplied);
  const authorized = actual.length === expected.length && timingSafeEqual(actual, expected);
  if (!authorized) return send(res, 401, { error: 'Invalid or missing local compiler token.' }, corsOrigin);
  if (req.method === 'GET' && url.pathname === '/api/status') {
    return send(res, 200, { connected: true, compilerInstalled: await fs.access(COMPILER).then(() => true, () => false), compilerPath: COMPILER }, corsOrigin);
  }
  if (req.method !== 'POST' || !['/api/check', '/api/run', '/api/build'].includes(url.pathname)) {
    return send(res, 404, { error: 'Not found.' }, corsOrigin);
  }

  let payload;
  try { payload = await readJson(req); }
  catch (error) { return send(res, error.status || 400, { error: error.message }, corsOrigin); }
  const { entryPath, files, stdin = '' } = payload;
  if (typeof stdin !== 'string' || Buffer.byteLength(stdin, 'utf8') > 64 * 1024) return send(res, 400, { error: 'stdin must be text no larger than 64 KiB.' }, corsOrigin);
  if (!safeRelativePath(entryPath) || !entryPath.endsWith('.plain')) return send(res, 400, { error: 'entryPath must be a safe relative .plain path.' }, corsOrigin);
  if (!files || typeof files !== 'object' || Array.isArray(files) || Object.keys(files).length > MAX_FILES) return send(res, 400, { error: `files must be an object with at most ${MAX_FILES} files.` }, corsOrigin);
  let total = 0;
  for (const [name, content] of Object.entries(files)) {
    if (!safeRelativePath(name) || typeof content !== 'string') return send(res, 400, { error: `Invalid workspace file: ${name}` }, corsOrigin);
    total += Buffer.byteLength(content, 'utf8');
    if (total > MAX_BODY) return send(res, 413, { error: 'Workspace exceeds the 2 MiB limit.' }, corsOrigin);
  }
  if (typeof files[entryPath] !== 'string') return send(res, 400, { error: 'Entry file is missing from workspace files.' }, corsOrigin);
  if (!await fs.access(COMPILER).then(() => true, () => false)) return send(res, 503, { error: `Plain compiler not found at ${COMPILER}. Run the platform setup script first.` }, corsOrigin);

  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'plain-ide-'));
  try {
    for (const [name, content] of Object.entries(files)) {
      const destination = path.resolve(temp, ...name.split('/'));
      if (!destination.startsWith(temp + path.sep)) throw new Error('Workspace path escaped its temporary directory.');
      await fs.mkdir(path.dirname(destination), { recursive: true });
      await fs.writeFile(destination, content, 'utf8');
    }
    const mode = url.pathname.slice('/api/'.length);
    const args = mode === 'check' ? ['check', entryPath] : mode === 'build' ? ['build', entryPath] : [entryPath];
    const result = await runCompiler(args, temp, mode === 'run' ? stdin : '');
    let artifact;
    if (mode === 'build' && result.code === 0) {
      const outputBase = entryPath.replace(/\.plain$/i, '');
      const outputName = IS_WIN ? `${outputBase}.exe` : outputBase;
      const artifactPath = path.resolve(temp, ...outputName.split('/'));
      const bytes = await fs.readFile(artifactPath);
      artifact = { name: path.basename(outputName), base64: bytes.toString('base64') };
    }
    return send(res, 200, { ok: result.code === 0, exitCode: result.code, stdout: result.stdout, stderr: result.stderr, artifact }, corsOrigin);
  } catch (error) {
    return send(res, 500, { ok: false, error: error.message }, corsOrigin);
  } finally {
    await fs.rm(temp, { recursive: true, force: true }).catch(() => {});
  }
});

server.listen(PORT, HOST, () => {
  console.log(`Plain IDE native compiler bridge listening at http://${HOST}:${PORT}`);
  console.log(`Platform: ${process.platform}/${process.arch}`);
  console.log(`Compiler: ${COMPILER}`);
  console.log(`IDE connection token: ${TOKEN}`);
  console.log('Keep this terminal open while using the native compiler.');
});
