(() => {
  const HOST_FORMDATA_BODY_HEADER = "x-rquickjs-host-body-formdata-v1";
  const { nextTick, normalizeMethod, parseBodyInit, Headers, stringToArrayBuffer } = globalThis.__web;
  const EVENTED_XHR_PENDING = new Map();
  const prevHttpComplete = globalThis.__host_runtime_http_complete;

  globalThis.__host_runtime_http_complete = function __host_runtime_http_complete(requestId, payloadRaw) {
    const pending = EVENTED_XHR_PENDING.get(Number(requestId));
    if (!pending) {
      if (typeof prevHttpComplete === "function") prevHttpComplete(requestId, payloadRaw);
      return;
    }
    EVENTED_XHR_PENDING.delete(Number(requestId));

    const { xhr, fail } = pending;
    const payload = JSON.parse(String(payloadRaw || "{}"));

    if (!payload.ok) {
      fail("error");
      return;
    }

    xhr._requestId = null;
    xhr.status = payload.status || 0;
    xhr.statusText = payload.statusText || "";
    xhr.responseURL = payload.url || xhr._url;
    xhr.offloaded = payload.offloaded === true;
    xhr.nativeBufferId = xhr.offloaded ? Number(payload.nativeBufferId || 0) : null;
    xhr.offloadedBytes = Number(payload.offloadedBytes || 0);
    xhr.wasiApplied = payload.wasiApplied === true;
    xhr.wasiNeedJsProcessing = payload.wasiNeedJsProcessing === true;
    xhr.wasiFunction = payload.wasiFunction || null;
    xhr.wasiOutputType = payload.wasiOutputType || null;
    xhr._responseHeaders = new Headers(payload.headers || {});
    xhr._setReadyState(XMLHttpRequest.HEADERS_RECEIVED);
    xhr.responseText = String(payload.body || "");
    xhr._setReadyState(XMLHttpRequest.LOADING);
    xhr._setReadyState(XMLHttpRequest.DONE);
    xhr.dispatchEvent({ type: "load", target: xhr });
    xhr.dispatchEvent({ type: "loadend", target: xhr });
  };

  class MiniEventTarget {
    constructor() {
      this._listeners = new Map();
    }

    addEventListener(type, listener) {
      if (typeof listener !== "function") return;
      const list = this._listeners.get(type) || [];
      list.push(listener);
      this._listeners.set(type, list);
    }

    removeEventListener(type, listener) {
      const list = this._listeners.get(type);
      if (!list) return;
      this._listeners.set(type, list.filter((it) => it !== listener));
    }

    dispatchEvent(event) {
      const list = this._listeners.get(event.type) || [];
      for (const listener of list) listener.call(this, event);
      const handler = this[`on${event.type}`];
      if (typeof handler === "function") handler.call(this, event);
      return true;
    }
  }

  class XMLHttpRequest extends MiniEventTarget {
    constructor() {
      super();
      this.readyState = XMLHttpRequest.UNSENT;
      this.status = 0;
      this.statusText = "";
      this.responseText = "";
      this.responseURL = "";
      this.responseType = "";
      this.timeout = 0;
      this.withCredentials = false;
      this.upload = new MiniEventTarget();
      this._method = null;
      this._url = null;
      this._headers = new Headers();
      this._responseHeaders = new Headers();
      this._aborted = false;
      this._sent = false;
      this._requestId = null;
      this.offloaded = false;
      this.nativeBufferId = null;
      this.offloadedBytes = 0;
      this.wasiApplied = false;
      this.wasiNeedJsProcessing = false;
      this.wasiFunction = null;
      this.wasiOutputType = null;
    }

    open(method, url, async = true) {
      if (!async) {
        throw new TypeError("当前实现仅支持异步 XMLHttpRequest");
      }
      this._method = normalizeMethod(method);
      this._url = String(url);
      this._aborted = false;
      this._sent = false;
      this.responseText = "";
      this.status = 0;
      this.statusText = "";
      this.offloaded = false;
      this.nativeBufferId = null;
      this.offloadedBytes = 0;
      this.wasiApplied = false;
      this.wasiNeedJsProcessing = false;
      this.wasiFunction = null;
      this.wasiOutputType = null;
      this._headers = new Headers();
      this._responseHeaders = new Headers();
      this._setReadyState(XMLHttpRequest.OPENED);
    }

    setRequestHeader(name, value) {
      if (this.readyState !== XMLHttpRequest.OPENED || this._sent) {
        throw new TypeError("setRequestHeader 调用时机不正确");
      }
      this._headers.append(name, value);
    }

    getResponseHeader(name) {
      if (this.readyState < XMLHttpRequest.HEADERS_RECEIVED) return null;
      return this._responseHeaders.get(name);
    }

    getAllResponseHeaders() {
      if (this.readyState < XMLHttpRequest.HEADERS_RECEIVED) return "";
      let out = "";
      this._responseHeaders.forEach((value, key) => {
        out += `${key}: ${value}\r\n`;
      });
      return out;
    }

    abort() {
      this._aborted = true;
      if (this._requestId !== null) {
        EVENTED_XHR_PENDING.delete(this._requestId);
        try {
          if (typeof globalThis.__http_request_drop_evented === "function") {
            globalThis.__http_request_drop_evented(this._requestId);
          } else {
            globalThis.__http_request_drop(this._requestId);
          }
        } catch (_err) {
        }
        this._requestId = null;
      }
      if (this.readyState === XMLHttpRequest.UNSENT || this.readyState === XMLHttpRequest.DONE) {
        return;
      }
      this._setReadyState(XMLHttpRequest.DONE);
      this.dispatchEvent({ type: "abort", target: this });
      this.dispatchEvent({ type: "loadend", target: this });
    }

    send(body) {
      if (this.readyState !== XMLHttpRequest.OPENED) {
        throw new TypeError("请先调用 open");
      }
      if (this._sent) {
        throw new TypeError("send 不能重复调用");
      }
      this._sent = true;

      nextTick(() => {
        if (this._aborted) return;
        const start = Date.now();
        const fail = (type) => {
          this._requestId = null;
          this._setReadyState(XMLHttpRequest.DONE);
          this.dispatchEvent({ type, target: this });
          this.dispatchEvent({ type: "loadend", target: this });
        };

        const dropPending = () => {
          if (this._requestId === null) return;
          try {
            globalThis.__http_request_drop_evented(this._requestId);
          } catch (_err) {
          }
          this._requestId = null;
        };

        try {
          const bodyInit = parseBodyInit(body);
          if (!this._headers.has("content-type") && bodyInit.contentType) {
            this._headers.set("content-type", bodyInit.contentType);
          }
          if (bodyInit.hostBodyKind === "formData") {
            this._headers.set(HOST_FORMDATA_BODY_HEADER, "1");
          }
          const startedRaw = globalThis.__http_request_start_evented(
            this._method,
            this._url,
            JSON.stringify(this._headers.toObject()),
            bodyInit.bodyText ?? null,
          );
          const started = JSON.parse(startedRaw);
          if (!started.ok) {
            fail("error");
            return;
          }

          this._requestId = Number(started.id);
          EVENTED_XHR_PENDING.set(this._requestId, { xhr: this, fail });
        } catch (_err) {
          dropPending();
          fail("error");
        }
      });
    }

    _setReadyState(state) {
      this.readyState = state;
      this.dispatchEvent({ type: "readystatechange", target: this });
    }

    get response() {
      if (this.responseType === "" || this.responseType === "text") {
        return this.responseText;
      }
      if (this.responseType === "json") {
        if (!this.responseText) return null;
        try {
          return JSON.parse(this.responseText);
        } catch (_err) {
          return null;
        }
      }
      if (this.responseType === "arraybuffer") {
        return stringToArrayBuffer(this.responseText);
      }
      return this.responseText;
    }
  }

  XMLHttpRequest.UNSENT = 0;
  XMLHttpRequest.OPENED = 1;
  XMLHttpRequest.HEADERS_RECEIVED = 2;
  XMLHttpRequest.LOADING = 3;
  XMLHttpRequest.DONE = 4;

  globalThis.__web.XMLHttpRequest = XMLHttpRequest;
})();
