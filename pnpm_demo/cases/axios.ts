import axios from "axios";

(() => {
  async function main(config: unknown = {}) {
    const cfg = (config || {}) as { baseUrl?: string };
    const baseUrl = String(cfg.baseUrl || "");
    if (!baseUrl || typeof XMLHttpRequest === "undefined") {
      return { ok: true, skipped: true, reason: "node-no-xhr-or-base-url" };
    }
    const res = await axios.get<{ path?: string; method?: string }>(`${baseUrl}/axios-case`, {
      adapter: "xhr",
    });
    return {
      ok: res.status === 200 && res.data.path === "/axios-case" && res.data.method === "GET",
    };
  }
  globalThis.__caseMain = main as (config?: unknown) => Promise<unknown>;
})();
