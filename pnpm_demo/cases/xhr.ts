(() => {
  async function main(config: unknown = {}) {
    const cfg = (config || {}) as { baseUrl?: string };
    const baseUrl = String(cfg.baseUrl || "");
    if (!baseUrl || typeof XMLHttpRequest === "undefined") {
      return { ok: true, skipped: true, reason: "node-no-xhr-or-base-url" };
    }

    const result = await new Promise<{ status: number; text: string }>((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open("GET", `${baseUrl}/xhr-case`);
      xhr.onload = () => resolve({ status: xhr.status, text: xhr.responseText });
      xhr.onerror = () => reject(new Error("xhr failed"));
      xhr.send();
    });

    const data = JSON.parse(result.text) as { path?: string; method?: string };
    return {
      ok: result.status === 200 && data.path === "/xhr-case" && data.method === "GET",
    };
  }
  globalThis.__caseMain = main as (config?: unknown) => Promise<unknown>;
})();
