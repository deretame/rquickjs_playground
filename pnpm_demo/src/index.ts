export default async function main() {
  const pathApi = (globalThis as unknown as { path?: { join?: (...parts: string[]) => string } }).path;
  const joinPath = pathApi && typeof pathApi.join === "function"
    ? (...parts: string[]) => pathApi.join!(...parts)
    : (...parts: string[]) => parts.join("/").replace(/\/+/g, "/").replace("/images/../", "/");
  const joined = joinPath("/demo", "images", "..", "out.png");

  const persistentStoreApi = (globalThis as unknown as {
    persistentStore?: {
      flushPersistentStore?: (key: string, value: string) => Promise<string>;
      loadPersistentStore?: (key: string, value: string) => Promise<string>;
    };
  }).persistentStore;

  let persistentStoreExample: unknown = null;
  if (
    persistentStoreApi
    && typeof persistentStoreApi.flushPersistentStore === "function"
    && typeof persistentStoreApi.loadPersistentStore === "function"
  ) {
    await persistentStoreApi.flushPersistentStore("demo.token", "abc123");
    const loaded = await persistentStoreApi.loadPersistentStore("demo.token", "");
    persistentStoreExample = { key: "demo.token", loadedRaw: loaded };
  }

  if (!globalThis.native || !globalThis.wasi) {
    return {
      ok: true,
      runtime: "node",
      joined,
      persistentStoreExample,
      note: "native/wasi 不存在，走 Node 回退路径",
    };
  }

  const input = new Uint8Array([1, 2, 3, 4]);
  const nativeApi = globalThis.native;
  const wasiApi = globalThis.wasi;
  const out = await nativeApi.chain(["invert", "invert"], input);

  const moduleBytes = new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
    0x03, 0x02, 0x01, 0x00,
    0x07, 0x0a, 0x01, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74, 0x00, 0x00,
    0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
  ]);
  const run = await wasiApi.run(moduleBytes);
  const stdout = await wasiApi.takeStdout(run);
  const stderr = await wasiApi.takeStderr(run);

  return {
    ok: true,
    joined,
    persistentStoreExample,
    out: Array.from(out),
    wasi: {
      exitCode: run.exitCode,
      stdoutLen: stdout.length,
      stderrLen: stderr.length,
    },
  };
}

if (typeof process !== "undefined" && process.versions && process.versions.node) {
  main()
    .then((value) => {
      console.log(JSON.stringify(value));
    })
    .catch((err: unknown) => {
      const message = err instanceof Error && err.stack ? err.stack : String(err);
      console.error(message);
      process.exitCode = 1;
    });
}
