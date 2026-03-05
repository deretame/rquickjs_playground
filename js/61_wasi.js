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
    return new Promise((resolve, reject) => {
      let requestId = null;

      const poll = () => {
        let step;
        try {
          step = JSON.parse(globalThis.__wasi_run_try_take(requestId));
        } catch (err) {
          reject(err);
          return;
        }

        if (!step.ok) {
          reject(new Error(step.error || "wasi 调用失败"));
          return;
        }

        if (!step.done) {
          setTimeout(poll, 0);
          return;
        }

        try {
          const res = parseHost(step.result || "{}");
          resolve({
            exitCode: Number(res.exitCode || 0),
            stdoutId: Number(res.stdoutId),
            stderrId: Number(res.stderrId),
          });
        } catch (err) {
          reject(err);
        }
      };

      try {
        const started = parseHost(globalThis.__wasi_run_start(Number(moduleId), stdinId, argsJson, consumeModule));
        requestId = Number(started.id);
        setTimeout(poll, 0);
      } catch (err) {
        if (requestId !== null) {
          try {
            globalThis.__wasi_run_drop(requestId);
          } catch (_dropErr) {
          }
        }
        reject(err);
      }
    });
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
