(() => {
  function parseHost(raw) {
    const payload = JSON.parse(raw);
    if (!payload.ok) {
      throw new Error(payload.error || "wasi 调用失败");
    }
    return payload;
  }

  async function runById(moduleId, options = {}) {
    const stdinId = options.stdinId === undefined || options.stdinId === null
      ? null
      : Number(options.stdinId);
    const argsJson = options.args === undefined ? null : JSON.stringify(options.args);
    const consumeModule = options.reuseModule ? false : true;
    const res = parseHost(globalThis.__wasi_run(Number(moduleId), stdinId, argsJson, consumeModule));
    return {
      exitCode: Number(res.exitCode || 0),
      stdoutId: Number(res.stdoutId),
      stderrId: Number(res.stderrId),
    };
  }

  async function run(moduleBytes, options = {}) {
    const moduleId = await globalThis.native.put(moduleBytes);
    return runById(moduleId, options);
  }

  async function takeStdout(result) {
    return globalThis.native.take(result.stdoutId);
  }

  async function takeStderr(result) {
    return globalThis.native.take(result.stderrId);
  }

  globalThis.__web.wasi = {
    run,
    runById,
    takeStdout,
    takeStderr,
  };
})();
