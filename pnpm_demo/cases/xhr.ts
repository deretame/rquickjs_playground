(() => {
  async function main(config: unknown = {}) {
    const cfg = (config || {}) as { baseUrl?: string };
    const baseUrl = String(cfg.baseUrl || "");
    if (!baseUrl || typeof XMLHttpRequest === "undefined") {
      return { ok: false, reason: "xhr-or-base-url-missing" };
    }

    const result = await new Promise<{ status: number; text: string }>((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open("GET", `${baseUrl}/xhr-case`);
      xhr.onload = () => resolve({ status: xhr.status, text: xhr.responseText });
      xhr.onerror = () => reject(new Error("xhr failed"));
      xhr.send();
    });

    const data = JSON.parse(result.text) as { path?: string; method?: string };

    const urlRes = await new Promise<{ status: number; text: string }>((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open("POST", `${baseUrl}/xhr-url`);
      xhr.onload = () => resolve({ status: xhr.status, text: xhr.responseText });
      xhr.onerror = () => reject(new Error("xhr url failed"));
      const params = new URLSearchParams();
      params.append("name", "quickjs");
      params.append("lang", "rust");
      xhr.send(params);
    });
    const urlData = JSON.parse(urlRes.text) as {
      method?: string;
      body?: string;
      headers?: Record<string, string>;
    };

    const formRes = await new Promise<{ status: number; text: string }>((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open("POST", `${baseUrl}/xhr-form`);
      xhr.onload = () => resolve({ status: xhr.status, text: xhr.responseText });
      xhr.onerror = () => reject(new Error("xhr form failed"));
      const fd = new FormData();
      fd.append("name", "quickjs");
      fd.append("upload", new Blob(["hello"], { type: "text/plain" }), "a.txt");
      xhr.send(fd);
    });
    const formData = JSON.parse(formRes.text) as {
      method?: string;
      body?: string;
      headers?: Record<string, string>;
    };

    return {
      ok:
        result.status === 200
        && data.path === "/xhr-case"
        && data.method === "GET"
        && urlRes.status === 200
        && urlData.method === "POST"
        && urlData.body === "name=quickjs&lang=rust"
        && String(urlData.headers?.["content-type"] || "")
          .includes("application/x-www-form-urlencoded;charset=UTF-8")
        && formRes.status === 200
        && formData.method === "POST"
        && String(formData.headers?.["content-type"] || "").includes("multipart/form-data;")
        && String(formData.body || "").includes("name=\"upload\"; filename=\"a.txt\""),
    };
  }
  globalThis.__caseMain = main as (config?: unknown) => Promise<unknown>;
})();
