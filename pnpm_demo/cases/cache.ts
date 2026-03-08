import { getApi, requireApi } from "../src/runtime-api";

export default async function main() {
  const maybeCache = getApi("cache");
  if (!maybeCache) {
    return { ok: false, reason: "cache-missing" };
  }

  const cache = requireApi("cache");
  const pluginCache = cache.scoped("caseCache");

    const numKey = "num";
    const lockKey = "lock";

    pluginCache.delete(numKey);
    pluginCache.delete(lockKey);
    pluginCache.clearAll();
    pluginCache.set(numKey, 1);
    const n1 = pluginCache.get(numKey);
    const insertedA = pluginCache.setIfAbsent(lockKey, { v: 1 });
    const insertedB = pluginCache.setIfAbsent(lockKey, { v: 2 });
    const casFail = pluginCache.compareAndSet(lockKey, { v: 2 }, { v: 3 });
    const casOk = pluginCache.compareAndSet(lockKey, { v: 1 }, { v: 3 });
    const lockV = pluginCache.get<{ v: number }>(lockKey, { v: -1 });
    const hasNum = pluginCache.has(numKey);
    const deleted = pluginCache.delete(numKey);
    const n2 = pluginCache.get(numKey, -1);
    const hasClear = typeof (cache as { clear?: unknown }).clear === "function";
    pluginCache.set("tempA", 1);
    pluginCache.set("tempB", 2);
    const cleared = pluginCache.clearAll();
    const hasTempA = pluginCache.has("tempA");
    const hasTempB = pluginCache.has("tempB");

  return {
    ok:
      n1 === 1
      && insertedA === true
      && insertedB === false
      && casFail === false
      && casOk === true
      && lockV.v === 3
      && hasNum === true
      && deleted === true
      && n2 === -1
      && hasClear === false
      && cleared >= 2
      && hasTempA === false
      && hasTempB === false,
  };
}
