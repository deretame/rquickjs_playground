(() => {
  const registry = new Map();

  function asPlainObject(input) {
    if (!input || typeof input !== "object" || Array.isArray(input)) {
      throw new TypeError("plugin info 必须是对象");
    }
    return JSON.parse(JSON.stringify(input));
  }

  function normalizePluginInfo(input) {
    const plain = asPlainObject(input);
    const name = String(plain.name || "").trim();
    const version = String(plain.version || "").trim();
    const apiVersionRaw = plain.apiVersion === undefined ? 1 : Number(plain.apiVersion);

    if (!name) throw new TypeError("plugin.name 不能为空");
    if (!version) throw new TypeError("plugin.version 不能为空");
    if (!Number.isInteger(apiVersionRaw) || apiVersionRaw <= 0) {
      throw new TypeError("plugin.apiVersion 必须是正整数");
    }

    plain.name = name;
    plain.version = version;
    plain.apiVersion = apiVersionRaw;
    return plain;
  }

  function register(info) {
    const normalized = normalizePluginInfo(info);
    registry.set(normalized.name, normalized);
    return { ...normalized };
  }

  function getInfo(name) {
    const key = String(name || "").trim();
    if (!key) return null;
    const found = registry.get(key);
    return found ? { ...found } : null;
  }

  function list() {
    return Array.from(registry.values()).map((item) => ({ ...item }));
  }

  function clear() {
    registry.clear();
  }

  globalThis.__plugin_host_get_info = function __pluginHostGetInfo(name) {
    return getInfo(name);
  };

  globalThis.__plugin_host_list = function __pluginHostList() {
    return list();
  };

  globalThis.__web.plugin = {
    register,
    getInfo,
    list,
    clear,
  };
})();
