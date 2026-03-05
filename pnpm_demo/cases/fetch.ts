(() => {
  async function main(config: unknown = {}) {
    const cfg = (config || {}) as { baseUrl?: string };
    const baseUrl = String(cfg.baseUrl || "");
    if (!baseUrl || typeof fetch !== "function") {
      return { ok: true, skipped: true, reason: "node-no-base-url-or-fetch" };
    }
    const res = await fetch(`${baseUrl}/fetch-case`);
    const data = await res.json() as { path?: string; method?: string };
    return {
      ok: res.status === 200 && data.path === "/fetch-case" && data.method === "GET",
    };
  }
  globalThis.__caseMain = main as (config?: unknown) => Promise<unknown>;
})();
