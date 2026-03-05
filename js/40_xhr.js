(() => {
  const { nextTick, normalizeMethod, parseBodyValue, Headers, stringToArrayBuffer } = globalThis.__web;

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

        try {
          const raw = globalThis.__http_request(
            this._method,
            this._url,
            JSON.stringify(this._headers.toObject()),
            parseBodyValue(body) ?? null,
          );
          const payload = JSON.parse(raw);

          if (!payload.ok) {
            this._setReadyState(XMLHttpRequest.DONE);
            this.dispatchEvent({ type: "error", target: this });
            this.dispatchEvent({ type: "loadend", target: this });
            return;
          }

          if (this.timeout > 0 && Date.now() - start > this.timeout) {
            this._setReadyState(XMLHttpRequest.DONE);
            this.dispatchEvent({ type: "timeout", target: this });
            this.dispatchEvent({ type: "loadend", target: this });
            return;
          }

          this.status = payload.status || 0;
          this.statusText = payload.statusText || "";
          this.responseURL = payload.url || this._url;
          this._responseHeaders = new Headers(payload.headers || {});
          this._setReadyState(XMLHttpRequest.HEADERS_RECEIVED);

          this.responseText = String(payload.body || "");
          this._setReadyState(XMLHttpRequest.LOADING);
          this._setReadyState(XMLHttpRequest.DONE);

          this.dispatchEvent({ type: "load", target: this });
          this.dispatchEvent({ type: "loadend", target: this });
        } catch (_err) {
          this._setReadyState(XMLHttpRequest.DONE);
          this.dispatchEvent({ type: "error", target: this });
          this.dispatchEvent({ type: "loadend", target: this });
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
