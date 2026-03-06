export {};

type NativeChainStep = string | { op: string; extraInputId?: number };

interface NativeApi {
  chain(steps: NativeChainStep[], input: Uint8Array | number): Promise<Uint8Array>;
  run(op: string, input: Uint8Array, args?: unknown, extraInput?: Uint8Array | number): Promise<Uint8Array>;
  put(input: Uint8Array): Promise<number>;
  free(id: number): Promise<void>;
}

interface WasiRunResult {
  exitCode: number;
  stdoutId: number;
  stderrId: number;
}

interface WasiApi {
  run(moduleBytes: Uint8Array, options?: { stdinId?: number; args?: string[]; reuseModule?: boolean }): Promise<WasiRunResult>;
  runById(moduleId: number, options?: { stdinId?: number; args?: string[]; reuseModule?: boolean }): Promise<WasiRunResult>;
  takeStdout(result: WasiRunResult): Promise<Uint8Array>;
  takeStderr(result: WasiRunResult): Promise<Uint8Array>;
}

interface CacheApi {
  set(key: string, value: unknown): unknown;
  setIfAbsent(key: string, value: unknown): boolean;
  compareAndSet(key: string, expected: unknown, value: unknown): boolean;
  get<T = unknown>(key: string, fallback?: T): T | unknown;
  has(key: string): boolean;
  delete(key: string): boolean;
  scoped(pluginName: string): Omit<CacheApi, "scoped"> & { clearAll(): number };
}

interface BridgeApi {
  call(name: string, ...args: unknown[]): Promise<unknown>;
}

interface CryptoHash {
  update(data: string | ArrayBuffer | ArrayBufferView, inputEncoding?: "utf8" | "utf-8" | "hex" | "base64" | "latin1" | "binary"): CryptoHash;
  digest(encoding?: "hex" | "base64" | "latin1" | "binary" | "utf8" | "utf-8" | "buffer"): string | Buffer;
}

interface CryptoApi {
  createHash(algorithm: "sha256" | "sha-256"): CryptoHash;
  createHmac(algorithm: "sha256" | "sha-256", key: string | ArrayBuffer | ArrayBufferView): CryptoHash;
  randomBytes(size: number): Buffer;
}

declare global {
  var native: NativeApi | undefined;
  var wasi: WasiApi | undefined;
  var cache: CacheApi | undefined;
  var bridge: BridgeApi | undefined;
  var crypto: CryptoApi | undefined;
  var nodeCryptoCompat: CryptoApi | undefined;
  var uuidv4: (() => string) | undefined;
  var __caseMain: ((config?: unknown) => Promise<unknown>) | undefined;
  var __demoMain: (() => Promise<unknown>) | undefined;
}
