(() => {
  const {
    parseBodyValue,
    stringToArrayBuffer,
    normalizeMethod,
    Headers,
  } = globalThis.__web;

  class BodyMixin {
    _initBody(bodyText) {
      this._bodyText = bodyText || "";
      this.bodyUsed = false;
    }

    _consumeBody() {
      if (this.bodyUsed) {
        return Promise.reject(new TypeError("Body 已被读取"));
      }
      this.bodyUsed = true;
      return Promise.resolve(this._bodyText);
    }

    text() {
      return this._consumeBody();
    }

    json() {
      return this._consumeBody().then((text) => JSON.parse(text));
    }

    arrayBuffer() {
      return this._consumeBody().then((text) => stringToArrayBuffer(text));
    }
  }

  class Request extends BodyMixin {
    constructor(input, init = {}) {
      super();

      if (input instanceof Request) {
        this.url = input.url;
        this.method = input.method;
        this.headers = new Headers(input.headers);
        this._initBody(input._bodyText);
      } else {
        this.url = String(input);
        this.method = "GET";
        this.headers = new Headers();
        this._initBody("");
      }

      this.method = normalizeMethod(init.method || this.method);
      if (init.headers) this.headers = new Headers(init.headers);
      this.signal = init.signal || null;
      this.credentials = init.credentials || "same-origin";
      this.mode = init.mode || null;
      this.redirect = init.redirect || "follow";
      this.referrer = init.referrer || "about:client";
      this.referrerPolicy = init.referrerPolicy || "";
      this.integrity = init.integrity || "";
      this.keepalive = Boolean(init.keepalive);
      this.cache = init.cache || "default";

      const bodyValue = parseBodyValue(init.body);
      if (bodyValue !== undefined) {
        if (this.method === "GET" || this.method === "HEAD") {
          throw new TypeError("GET/HEAD 请求不能带 body");
        }
        this._initBody(bodyValue);
        if (!this.headers.has("content-type") && typeof init.body === "object") {
          this.headers.set("content-type", "application/json");
        }
      }
    }
  }

  class Response extends BodyMixin {
    constructor(body = "", init = {}) {
      super();
      this._initBody(String(body));
      this.status = init.status || 200;
      this.statusText = init.statusText || "OK";
      this.headers = new Headers(init.headers || {});
      this.url = init.url || "";
      this.ok = this.status >= 200 && this.status < 300;
    }

    clone() {
      if (this.bodyUsed) {
        throw new TypeError("Body 已被读取，无法 clone");
      }
      return new Response(this._bodyText, {
        status: this.status,
        statusText: this.statusText,
        headers: this.headers,
        url: this.url,
      });
    }

    static json(data, init = {}) {
      const headers = new Headers(init.headers || {});
      if (!headers.has("content-type")) {
        headers.set("content-type", "application/json");
      }
      return new Response(JSON.stringify(data), {
        ...init,
        headers,
      });
    }
  }

  function fetch(input, init = {}) {
    const request = input instanceof Request ? new Request(input, init) : new Request(input, init);

    return new Promise((resolve, reject) => {
      try {
        if (request.signal && request.signal.aborted) {
          const err = new Error(request.signal.reason || "请求已取消");
          err.name = "AbortError";
          reject(err);
          return;
        }

        const raw = globalThis.__http_request(
          request.method,
          request.url,
          JSON.stringify(request.headers.toObject()),
          request._bodyText || null,
        );
        const payload = JSON.parse(raw);

        if (!payload.ok) {
          reject(new TypeError(payload.error || "网络请求失败"));
          return;
        }

        resolve(
          new Response(payload.body || "", {
            status: payload.status,
            statusText: payload.statusText,
            headers: payload.headers || {},
            url: payload.url || request.url,
          }),
        );
      } catch (err) {
        reject(err);
      }
    });
  }

  globalThis.__web.Request = Request;
  globalThis.__web.Response = Response;
  globalThis.__web.fetch = fetch;
})();
