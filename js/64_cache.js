(() => {
  function parseHost(raw) {
    const payload = JSON.parse(raw);
    if (!payload.ok) {
      throw new Error(payload.error || "cache 调用失败");
    }
    return payload;
  }

  const cache = {
    set(key, value) {
      if (typeof key !== "string") {
        throw new TypeError("cache key 必须是字符串");
      }
      parseHost(globalThis.__cache_set(key, JSON.stringify(value)));
      return value;
    },

    setIfAbsent(key, value) {
      if (typeof key !== "string") {
        throw new TypeError("cache key 必须是字符串");
      }
      const payload = parseHost(globalThis.__cache_set_if_absent(key, JSON.stringify(value)));
      return Boolean(payload.inserted);
    },

    compareAndSet(key, expected, value) {
      if (typeof key !== "string") {
        throw new TypeError("cache key 必须是字符串");
      }
      const payload = parseHost(globalThis.__cache_compare_and_set(
        key,
        JSON.stringify(expected),
        JSON.stringify(value),
      ));
      return Boolean(payload.updated);
    },

    get(key, fallback = null) {
      if (typeof key !== "string") {
        throw new TypeError("cache key 必须是字符串");
      }
      const payload = parseHost(globalThis.__cache_get(key));
      return payload.found ? payload.value : fallback;
    },

    has(key) {
      if (typeof key !== "string") {
        throw new TypeError("cache key 必须是字符串");
      }
      const payload = parseHost(globalThis.__cache_get(key));
      return Boolean(payload.found);
    },

    delete(key) {
      if (typeof key !== "string") {
        throw new TypeError("cache key 必须是字符串");
      }
      const payload = parseHost(globalThis.__cache_delete(key));
      return Boolean(payload.deleted);
    },

    clear() {
      parseHost(globalThis.__cache_clear());
    },
  };

  globalThis.__web.cache = cache;
})();
