"use strict";
(() => {
  // cases/fetch.ts
  (() => {
    async function main(config = {}) {
      const cfg = config || {};
      const baseUrl = String(cfg.baseUrl || "");
      if (!baseUrl || typeof fetch !== "function") {
        return { ok: true, skipped: true, reason: "node-no-base-url-or-fetch" };
      }
      const res = await fetch(`${baseUrl}/fetch-case`);
      const data = await res.json();
      return {
        ok: res.status === 200 && data.path === "/fetch-case" && data.method === "GET"
      };
    }
    globalThis.__caseMain = main;
  })();
})();
