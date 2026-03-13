(() => {
  const HOST_FORMDATA_BODY_HEADER = "x-rquickjs-host-body-formdata-v1";
  const EVENTED_HTTP_PENDING = new Map();
  const prevHttpComplete = globalThis.__host_runtime_http_complete;

  globalThis.__host_runtime_http_complete = function __host_runtime_http_complete(requestId, payloadRaw) {
    const pending = EVENTED_HTTP_PENDING.get(Number(requestId));
    if (!pending) {
      if (typeof prevHttpComplete === "function") prevHttpComplete(requestId, payloadRaw);
      return;
    }
    EVENTED_HTTP_PENDING.delete(Number(requestId));

    const { request, resolve, reject, finish, dropPending } = pending;

    let payload;
    try {
      payload = JSON.parse(String(payloadRaw || "{}"));
    } catch (err) {
      dropPending();
      finish(() => reject(err));
      return;
    }

    if (!payload.ok) {
      dropPending();
      finish(() => reject(new TypeError(payload.error || "网络请求失败")));
      return;
    }

    finish(() =>
      resolve(
        new Response(payload.body || "", {
          status: payload.status,
          statusText: payload.statusText,
          headers: payload.headers || {},
          url: payload.url || request.url,
          offloaded: payload.offloaded === true,
          nativeBufferId: payload.nativeBufferId,
          offloadedBytes: payload.offloadedBytes,
          wasiApplied: payload.wasiApplied === true,
          wasiNeedJsProcessing: payload.wasiNeedJsProcessing === true,
          wasiFunction: payload.wasiFunction || null,
          wasiOutputType: payload.wasiOutputType || null,
        }),
      ),
    );
  };

  const {
    parseBodyInit,
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
      if (this.offloaded === true && this.nativeBufferId !== null && this.nativeBufferId !== undefined) {
        if (!globalThis.native || typeof globalThis.native.take !== "function") {
          return Promise.reject(new TypeError("native.take 不可用，无法读取 offload 二进制数据"));
        }
        const id = Number(this.nativeBufferId);
        this.nativeBufferId = null;
        return globalThis.native.take(id).then((bytes) => {
          if (typeof TextDecoder === "function") {
            return new TextDecoder("utf-8").decode(bytes);
          }
          let text = "";
          for (let i = 0; i < bytes.length; i += 1) text += String.fromCharCode(bytes[i]);
          return text;
        });
      }
      return Promise.resolve(this._bodyText);
    }

    text() {
      return this._consumeBody();
    }

    json() {
      return this._consumeBody().then((text) => JSON.parse(text));
    }

    arrayBuffer() {
      if (this.bodyUsed) {
        return Promise.reject(new TypeError("Body 已被读取"));
      }
      if (this.offloaded === true && this.nativeBufferId !== null && this.nativeBufferId !== undefined) {
        if (!globalThis.native || typeof globalThis.native.take !== "function") {
          return Promise.reject(new TypeError("native.take 不可用，无法读取 offload 二进制数据"));
        }
        this.bodyUsed = true;
        const id = Number(this.nativeBufferId);
        this.nativeBufferId = null;
        return globalThis.native.take(id).then((bytes) => {
          const out = new Uint8Array(bytes.length);
          out.set(bytes);
          return out.buffer;
        });
      }
      return this._consumeBody().then((text) => {
        if (typeof TextEncoder === "function") {
          const bytes = new TextEncoder().encode(text);
          const out = new Uint8Array(bytes.length);
          out.set(bytes);
          return out.buffer;
        }
        return stringToArrayBuffer(text);
      });
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

      const bodyInit = parseBodyInit(init.body);
      if (bodyInit.bodyText !== undefined) {
        if (this.method === "GET" || this.method === "HEAD") {
          throw new TypeError("GET/HEAD 请求不能带 body");
        }
        this._initBody(bodyInit.bodyText);
        if (!this.headers.has("content-type") && bodyInit.contentType) {
          this.headers.set("content-type", bodyInit.contentType);
        }
        if (bodyInit.hostBodyKind === "formData") {
          this.headers.set(HOST_FORMDATA_BODY_HEADER, "1");
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
      this.offloaded = Boolean(init.offloaded);
      this.nativeBufferId = init.nativeBufferId === undefined || init.nativeBufferId === null
        ? null
        : Number(init.nativeBufferId);
      this.offloadedBytes = Number(init.offloadedBytes || 0);
      this.wasiApplied = Boolean(init.wasiApplied);
      this.wasiNeedJsProcessing = Boolean(init.wasiNeedJsProcessing);
      this.wasiFunction = init.wasiFunction || null;
      this.wasiOutputType = init.wasiOutputType || null;
    }

    clone() {
      if (this.bodyUsed) {
        throw new TypeError("Body 已被读取，无法 clone");
      }
      if (this.offloaded && this.nativeBufferId !== null) {
        throw new TypeError("offload 响应暂不支持 clone");
      }
      return new Response(this._bodyText, {
        status: this.status,
        statusText: this.statusText,
        headers: this.headers,
        url: this.url,
        offloaded: this.offloaded,
        nativeBufferId: this.nativeBufferId,
        offloadedBytes: this.offloadedBytes,
        wasiApplied: this.wasiApplied,
        wasiNeedJsProcessing: this.wasiNeedJsProcessing,
        wasiFunction: this.wasiFunction,
        wasiOutputType: this.wasiOutputType,
      });
    }

    async takeOffloadedBody() {
      if (this.bodyUsed) {
        throw new TypeError("Body 已被读取");
      }
      this.bodyUsed = true;

      if (!this.offloaded || this.nativeBufferId === null) {
        return new Uint8Array(0);
      }
      if (!globalThis.native || typeof globalThis.native.take !== "function") {
        throw new TypeError("native.take 不可用，无法读取 offload 二进制数据");
      }
      const id = this.nativeBufferId;
      this.nativeBufferId = null;
      return globalThis.native.take(id);
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
      let requestId = null;
      let settled = false;

      const finish = (cb) => {
        if (settled) return;
        settled = true;
        cb();
      };

      const dropPending = () => {
        if (requestId === null) return;
        try {
          globalThis.__http_request_drop_evented(requestId);
        } catch (_err) {
        }
      };

      try {
        if (request.signal && request.signal.aborted) {
          const err = new Error(request.signal.reason || "请求已取消");
          err.name = "AbortError";
          finish(() => reject(err));
          return;
        }

        const startedRaw = globalThis.__http_request_start_evented(
          request.method,
          request.url,
          JSON.stringify(request.headers.toObject()),
          request._bodyText || null,
        );
        const started = JSON.parse(startedRaw);
        if (!started.ok) {
          finish(() => reject(new TypeError(started.error || "网络请求失败")));
          return;
        }
        requestId = Number(started.id);

        const onAbort = () => {
          EVENTED_HTTP_PENDING.delete(requestId);
          dropPending();
          const err = new Error(request.signal.reason || "请求已取消");
          err.name = "AbortError";
          finish(() => reject(err));
        };
        if (request.signal) {
          request.signal.addEventListener("abort", onAbort);
        }

        EVENTED_HTTP_PENDING.set(requestId, {
          request,
          resolve,
          reject,
          finish,
          dropPending,
        });
      } catch (err) {
        dropPending();
        finish(() => reject(err));
      }
    });
  }

  globalThis.__web.Request = Request;
  globalThis.__web.Response = Response;
  globalThis.__web.fetch = fetch;
})();
