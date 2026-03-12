(() => {
  function parseHost(raw) {
    const payload = JSON.parse(raw);
    if (!payload.ok) {
      throw new Error(payload.error || "bridge 调用失败");
    }
    return payload.data;
  }

  async function call(name, ...args) {
    const raw = globalThis.__host_call(String(name), JSON.stringify(args));
    return parseHost(raw);
  }

  async function flushPersistentStore(key, value) {
    return call("flush_persistent_store", String(key), String(value));
  }

  async function loadPersistentStore(key, value) {
    return call("load_persistent_store", String(key), String(value));
  }

  globalThis.__web.bridge = {
    call,
  };

  globalThis.__web.persistentStore = {
    flushPersistentStore,
    loadPersistentStore,
  };
})();
