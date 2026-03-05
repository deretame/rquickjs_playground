"use strict";
var DemoApp = (() => {
  // src/index.ts
  (() => {
    async function main() {
      const pathApi = globalThis.path;
      const joinPath = pathApi && typeof pathApi.join === "function" ? (...parts) => pathApi.join(...parts) : (...parts) => parts.join("/").replace(/\/+/g, "/").replace("/images/../", "/");
      const joined = joinPath("/demo", "images", "..", "out.png");
      if (!globalThis.native || !globalThis.wasi) {
        return {
          ok: true,
          runtime: "node",
          joined,
          note: "native/wasi \u4E0D\u5B58\u5728\uFF0C\u8D70 Node \u56DE\u9000\u8DEF\u5F84"
        };
      }
      const input = new Uint8Array([1, 2, 3, 4]);
      const nativeApi = globalThis.native;
      const wasiApi = globalThis.wasi;
      const out = await nativeApi.chain(["invert", "invert"], input);
      const moduleBytes = new Uint8Array([
        0,
        97,
        115,
        109,
        1,
        0,
        0,
        0,
        1,
        4,
        1,
        96,
        0,
        0,
        3,
        2,
        1,
        0,
        7,
        10,
        1,
        6,
        95,
        115,
        116,
        97,
        114,
        116,
        0,
        0,
        10,
        4,
        1,
        2,
        0,
        11
      ]);
      const run = await wasiApi.run(moduleBytes);
      const stdout = await wasiApi.takeStdout(run);
      const stderr = await wasiApi.takeStderr(run);
      return {
        ok: true,
        joined,
        out: Array.from(out),
        wasi: {
          exitCode: run.exitCode,
          stdoutLen: stdout.length,
          stderrLen: stderr.length
        }
      };
    }
    globalThis.__demoMain = main;
    if (typeof process !== "undefined" && process.versions && process.versions.node) {
      main().then((value) => {
        console.log(JSON.stringify(value));
      }).catch((err) => {
        const message = err instanceof Error && err.stack ? err.stack : String(err);
        console.error(message);
        process.exitCode = 1;
      });
    }
  })();
})();
