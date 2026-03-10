import { getApi, requireApi } from "../src/runtime-api";

export default async function main() {
  const maybeCache = getApi("cache");
  if (!maybeCache) {
    return { ok: false, reason: "cache-missing" };
  }

  const cache = requireApi("cache");
  const withScope = (key: string) => `caseCache::${key}`;

    const numKey = withScope("num");
    const lockKey = withScope("lock");
    const tempAKey = withScope("tempA");
    const tempBKey = withScope("tempB");

    cache.delete(numKey);
    cache.delete(lockKey);
    cache.delete(tempAKey);
    cache.delete(tempBKey);
    cache.set(numKey, 1);
    const n1 = cache.get(numKey);
    const insertedA = cache.setIfAbsent(lockKey, { v: 1 });
    const insertedB = cache.setIfAbsent(lockKey, { v: 2 });
    const casFail = cache.compareAndSet(lockKey, { v: 2 }, { v: 3 });
    const casOk = cache.compareAndSet(lockKey, { v: 1 }, { v: 3 });
    const lockV = cache.get<{ v: number }>(lockKey, { v: -1 });
    const hasNum = cache.has(numKey);
    const deleted = cache.delete(numKey);
    const n2 = cache.get(numKey, -1);
    const hasClear = typeof (cache as { clear?: unknown }).clear === "function";
    cache.set(tempAKey, 1);
    cache.set(tempBKey, 2);
    const hasTempA_beforeDelete = cache.has(tempAKey);
    const hasTempB_beforeDelete = cache.has(tempBKey);
    cache.delete(tempAKey);
    cache.delete(tempBKey);
    const hasTempA = cache.has(tempAKey);
    const hasTempB = cache.has(tempBKey);

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
      && hasTempA_beforeDelete === true
      && hasTempB_beforeDelete === true
      && hasTempA === false
      && hasTempB === false,
  };
}
