"use strict";
(() => {
  // cases/xhr.ts
  (() => {
    async function main(config = {}) {
      const cfg = config || {};
      const baseUrl = String(cfg.baseUrl || "");
      if (!baseUrl || typeof XMLHttpRequest === "undefined") {
        return { ok: true, skipped: true, reason: "node-no-xhr-or-base-url" };
      }
      const result = await new Promise((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open("GET", `${baseUrl}/xhr-case`);
        xhr.onload = () => resolve({ status: xhr.status, text: xhr.responseText });
        xhr.onerror = () => reject(new Error("xhr failed"));
        xhr.send();
      });
      const data = JSON.parse(result.text);
      return {
        ok: result.status === 200 && data.path === "/xhr-case" && data.method === "GET"
      };
    }
    globalThis.__caseMain = main;
  })();
})();
