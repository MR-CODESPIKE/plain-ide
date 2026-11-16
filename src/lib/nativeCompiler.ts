const BRIDGE_URL = 'http://127.0.0.1:4317';
const TOKEN_KEY = 'plain-ide.native-compiler-token.v1';

export interface NativeCompilerResponse {
  ok: boolean;
  exitCode?: number;
  stdout?: string;
  stderr?: string;
  error?: string;
  artifact?: { name: string; base64: string };
}

export interface NativeWorkspaceFile {
  path: string;
  content: string;
}

export function getNativeCompilerToken(): string {
  try { return window.localStorage.getItem(TOKEN_KEY) ?? ''; }
  catch { return ''; }
}

export function setNativeCompilerToken(token: string): void {
  window.localStorage.setItem(TOKEN_KEY, token);
}

export function clearNativeCompilerToken(): void {
  window.localStorage.removeItem(TOKEN_KEY);
}

export async function checkNativeCompilerHealth(): Promise<{ compilerInstalled: boolean; platform: string; arch: string }> {
  const response = await fetch(`${BRIDGE_URL}/api/health`, { signal: AbortSignal.timeout(2500) });
  if (!response.ok) throw new Error(`Local compiler bridge returned HTTP ${response.status}.`);
  return response.json() as Promise<{ compilerInstalled: boolean; platform: string; arch: string }>;
}

export async function verifyNativeCompilerToken(token = getNativeCompilerToken()): Promise<boolean> {
  if (!token) return false;
  const response = await fetch(`${BRIDGE_URL}/api/status`, {
    headers: { Authorization: `Bearer ${token}` },
    signal: AbortSignal.timeout(2500),
  });
  return response.ok;
}

export async function invokeNativeCompiler(
  mode: 'check' | 'run' | 'build',
  entryPath: string,
  files: NativeWorkspaceFile[],
  stdin = '',
): Promise<NativeCompilerResponse> {
  const token = getNativeCompilerToken();
  if (!token) throw new Error('Connect the local Plain compiler first.');
  const fileMap = Object.fromEntries(files.map(file => [file.path, file.content]));
  const response = await fetch(`${BRIDGE_URL}/api/${mode}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ entryPath, files: fileMap, stdin }),
    signal: AbortSignal.timeout(60_000),
  });
  const result = await response.json() as NativeCompilerResponse;
  if (!response.ok) throw new Error(result.error || `Native compiler request failed (HTTP ${response.status}).`);
  return result;
}

export function downloadNativeArtifact(name: string, base64: string): void {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  const buffer = new ArrayBuffer(bytes.length);
  new Uint8Array(buffer).set(bytes);
  const url = URL.createObjectURL(new Blob([buffer], { type: 'application/octet-stream' }));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
}
