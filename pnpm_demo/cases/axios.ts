import axios from "axios";

(() => {
  async function main(config: unknown = {}) {
    const cfg = (config || {}) as { baseUrl?: string };
    const baseUrl = String(cfg.baseUrl || "");
    if (!baseUrl) {
      return { ok: false, reason: "base-url-missing" };
    }
    const res = await axios.get<{ path?: string; method?: string }>(`${baseUrl}/axios-case`, {
      adapter: "fetch",
      timeout: 3000,
    });
    return {
      ok: res.status === 200 && res.data.path === "/axios-case" && res.data.method === "GET",
    };
  }
  globalThis.__caseMain = main as (config?: unknown) => Promise<unknown>;
})();
