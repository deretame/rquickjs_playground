(() => {
  function hashPluginPrefix(pluginName) {
    const name = String(pluginName || "").trim();
    if (!name) {
      throw new TypeError("pluginName 不能为空");
    }
    if (name.length > 200) {
      throw new TypeError("pluginName 长度不能超过 200 字符");
    }
    const cryptoRef = globalThis.crypto;
    if (!cryptoRef || typeof cryptoRef.createHash !== "function") {
      throw new TypeError("crypto.createHash 不可用，无法生成插件前缀");
    }
    const digest = String(cryptoRef.createHash("sha256").update(name, "utf8").digest("hex"));
    const prefix = `plg_${digest}`;
    if (prefix.length > 200) {
      throw new TypeError("插件前缀长度超过 200 字符");
    }
    return prefix;
  }

  function normalizeLocalKey(key) {
    const raw = String(key || "").trim();
    if (!raw) {
      throw new TypeError("cache key 不能为空");
    }
    if (raw.length > 200) {
      throw new TypeError("cache key 长度不能超过 200 字符");
    }
    return raw;
  }

  function hashLocalKey(localKey) {
    const cryptoRef = globalThis.crypto;
    if (!cryptoRef || typeof cryptoRef.createHash !== "function") {
      throw new TypeError("crypto.createHash 不可用，无法生成 cache key 哈希");
    }
    return String(cryptoRef.createHash("sha256").update(localKey, "utf8").digest("hex"));
  }

  function validateKey(key) {
    if (typeof key !== "string") {
      throw new TypeError("cache key 必须是字符串");
    }
    const raw = key.trim();
    const idx = raw.indexOf("::");
    if (idx <= 0 || idx >= raw.length - 2) {
      throw new TypeError("cache key 必须为 {pluginPrefix}::{sha256(key)} 格式");
    }
    const prefix = raw.slice(0, idx);
    if (prefix.length > 200) {
      throw new TypeError("cache key 前缀长度不能超过 200 字符");
    }
    return raw;
  }

  function scoped(pluginName) {
    const prefix = hashPluginPrefix(pluginName);
    const withPrefix = (key) => {
      const localKey = normalizeLocalKey(key);
      const keyHash = hashLocalKey(localKey);
      return `${prefix}::${keyHash}`;
    };
    return {
      set(key, value) {
        return cache.set(withPrefix(key), value);
      },
      setIfAbsent(key, value) {
        return cache.setIfAbsent(withPrefix(key), value);
      },
      compareAndSet(key, expected, value) {
        return cache.compareAndSet(withPrefix(key), expected, value);
      },
      get(key, fallback = null) {
        return cache.get(withPrefix(key), fallback);
      },
      has(key) {
        return cache.has(withPrefix(key));
      },
      delete(key) {
        return cache.delete(withPrefix(key));
      },
      clearAll() {
        const payload = parseHost(globalThis.__cache_clear_prefix(prefix));
        return Number(payload.deleted || 0);
      },
    };
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
      const normalized = validateKey(key);
      parseHost(globalThis.__cache_set(normalized, JSON.stringify(value)));
      return value;
    },

    setIfAbsent(key, value) {
      const normalized = validateKey(key);
      const payload = parseHost(globalThis.__cache_set_if_absent(normalized, JSON.stringify(value)));
      return Boolean(payload.inserted);
    },

    compareAndSet(key, expected, value) {
      const normalized = validateKey(key);
      const payload = parseHost(globalThis.__cache_compare_and_set(
        normalized,
        JSON.stringify(expected),
        JSON.stringify(value),
      ));
      return Boolean(payload.updated);
    },

    get(key, fallback = null) {
      const normalized = validateKey(key);
      const payload = parseHost(globalThis.__cache_get(normalized));
      return payload.found ? payload.value : fallback;
    },

    has(key) {
      const normalized = validateKey(key);
      const payload = parseHost(globalThis.__cache_get(normalized));
      return Boolean(payload.found);
    },

    delete(key) {
      const normalized = validateKey(key);
      const payload = parseHost(globalThis.__cache_delete(normalized));
      return Boolean(payload.deleted);
    },
    scoped,
  };

  globalThis.__web.cache = cache;
})();
