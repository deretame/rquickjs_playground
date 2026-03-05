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

declare global {
  var native: NativeApi | undefined;
  var wasi: WasiApi | undefined;
  var __caseMain: ((config?: unknown) => Promise<unknown>) | undefined;
  var __demoMain: (() => Promise<unknown>) | undefined;
}
