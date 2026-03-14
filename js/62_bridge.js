(() => {
  function parseHost(raw) {
    const payload = JSON.parse(raw);
    if (!payload.ok) {
      throw new Error(payload.error || "bridge 调用失败");
    }
    return payload.data;
  }

  function toByteArray(input) {
    if (input instanceof Uint8Array) return input;
    if (ArrayBuffer.isView(input)) {
      return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    }
    if (input instanceof ArrayBuffer) {
      return new Uint8Array(input);
    }
    return null;
  }

  function normalizeArg(input) {
    const bytes = toByteArray(input);
    if (bytes) return Array.from(bytes);

    if (Array.isArray(input)) {
      return input.map((item) => normalizeArg(item));
    }

    if (input && typeof input === "object" && Object.getPrototypeOf(input) === Object.prototype) {
      const out = {};
      for (const [key, value] of Object.entries(input)) {
        out[key] = normalizeArg(value);
      }
      return out;
    }

    return input;
  }

  async function call(name, ...args) {
    const normalizedArgs = args.map((arg) => normalizeArg(arg));
    const raw = globalThis.__host_call(String(name), JSON.stringify(normalizedArgs));
    return parseHost(raw);
  }

  async function savePluginConfig(key, value) {
    return call("save_plugin_config", String(key), String(value));
  }

  async function loadPluginConfig(key, value) {
    return call("load_plugin_config", String(key), String(value));
  }

  async function gzipDecompress(input) {
    const out = await call("compression.gzip_decompress", input);
    return Uint8Array.from(Array.isArray(out) ? out : []);
  }

  async function gzipCompress(input) {
    const out = await call("compression.gzip_compress", input);
    return Uint8Array.from(Array.isArray(out) ? out : []);
  }

  globalThis.__web.bridge = {
    call,
    gzipDecompress,
    gzipCompress,
  };

  globalThis.__web.pluginConfig = {
    savePluginConfig,
    loadPluginConfig,
  };
})();
