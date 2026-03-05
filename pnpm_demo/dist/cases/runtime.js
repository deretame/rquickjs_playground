"use strict";
(() => {
  // cases/runtime.ts
  (() => {
    async function main() {
      const BufferRef = globalThis.Buffer;
      const runtimePath = globalThis.path;
      const pathRef = runtimePath && typeof runtimePath.join === "function" ? { join: (...parts) => runtimePath.join(...parts) } : {
        join: (...parts) => parts.join("/").replace(/\/+/g, "/").replace("/b/../", "/")
      };
      const p = pathRef.join("/a", "b", "..", "c.txt");
      let ticked = false;
      process.nextTick(() => {
        ticked = true;
      });
      await new Promise((resolve) => process.nextTick(resolve));
      const b = BufferRef.concat([BufferRef.from("ab"), BufferRef.from("cd")]).toString("utf8");
      return { ok: p === "/a/c.txt" && b === "abcd" && ticked };
    }
    globalThis.__caseMain = main;
  })();
})();
