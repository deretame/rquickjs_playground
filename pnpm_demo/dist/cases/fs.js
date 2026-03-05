"use strict";
(() => {
  // cases/fs.ts
  (() => {
    async function main(config = {}) {
      const cfg = config || {};
      const base = String(cfg.baseDir || ".");
      const runtimeFs = globalThis.fs;
      if (!runtimeFs || !runtimeFs.promises) {
        return { ok: true, skipped: true, reason: "node-no-runtime-fs" };
      }
      const file = `${base.replace(/\\/g, "/")}/case.txt`;
      await runtimeFs.promises.writeFile(file, "hello", "utf8");
      await runtimeFs.promises.appendFile(file, "-world", "utf8");
      const text = await runtimeFs.promises.readFile(file, "utf8");
      await runtimeFs.promises.rm(file, { force: true });
      return { ok: text === "hello-world" };
    }
    globalThis.__caseMain = main;
  })();
})();
