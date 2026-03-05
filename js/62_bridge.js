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

  globalThis.__web.bridge = {
    call,
  };
})();
