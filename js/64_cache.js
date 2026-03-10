(() => {
  function hasControlChars(input) {
    for (let i = 0; i < input.length; i += 1) {
      const code = input.charCodeAt(i);
      if (code <= 31 || code === 127) return true;
    }
    return false;
  }

  function normalizeLocalKey(key, label = "cache key") {
    const raw = String(key || "").trim();
    if (!raw) {
      throw new TypeError(`${label} 不能为空`);
    }
    if (raw.length > 240) {
      throw new TypeError(`${label} 长度不能超过 240 字符`);
    }
    if (hasControlChars(raw)) {
      throw new TypeError(`${label} 不能包含控制字符`);
    }
    return raw;
  }

  function parseHost(raw) {
    const payload = JSON.parse(raw);
    if (!payload.ok) {
      throw new Error(payload.error || "cache 调用失败");
    }
    return payload;
  }

  const cache = {
    set(key, value) {
      const normalized = normalizeLocalKey(key);
      parseHost(globalThis.__cache_set(normalized, JSON.stringify(value)));
      return value;
    },

    setIfAbsent(key, value) {
      const normalized = normalizeLocalKey(key);
      const payload = parseHost(globalThis.__cache_set_if_absent(normalized, JSON.stringify(value)));
      return Boolean(payload.inserted);
    },

    compareAndSet(key, expected, value) {
      const normalized = normalizeLocalKey(key);
      const payload = parseHost(globalThis.__cache_compare_and_set(
        normalized,
        JSON.stringify(expected),
        JSON.stringify(value),
      ));
      return Boolean(payload.updated);
    },

    get(key, fallback = null) {
      const normalized = normalizeLocalKey(key);
      const payload = parseHost(globalThis.__cache_get(normalized));
      return payload.found ? payload.value : fallback;
    },

    has(key) {
      const normalized = normalizeLocalKey(key);
      const payload = parseHost(globalThis.__cache_get(normalized));
      return Boolean(payload.found);
    },

    delete(key) {
      const normalized = normalizeLocalKey(key);
      const payload = parseHost(globalThis.__cache_delete(normalized));
      return Boolean(payload.deleted);
    },
  };

  globalThis.__web.cache = cache;
})();
