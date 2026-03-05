use crate::tests::run_async_script;
use crate::web_runtime::{
    call_js_global_function, plugin_get_info, plugin_list, plugin_load, WEB_POLYFILL,
};
use rquickjs::{Context, Runtime};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[test]
fn runtime_timers_and_microtask() {
    let script = r#"
      (async () => {
        const events = [];

        const timeoutId = setTimeout(() => events.push("timeout-cancelled"), 1);
        clearTimeout(timeoutId);

        queueMicrotask(() => events.push("micro"));

        let count = 0;
        const intervalId = setInterval(() => {
          count += 1;
          events.push(`interval-${count}`);
          if (count >= 2) clearInterval(intervalId);
        }, 1);

        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();

        return JSON.stringify({ events });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    let events = parsed["events"].as_array().expect("events 必须是数组");

    assert!(events.iter().any(|v| v == "micro"));
    assert!(events.iter().any(|v| v == "interval-1"));
    assert!(events.iter().any(|v| v == "interval-2"));
    assert!(!events.iter().any(|v| v == "timeout-cancelled"));
}

#[test]
fn runtime_text_and_base64() {
    let script = r#"
      (async () => {
        const te = new TextEncoder();
        const td = new TextDecoder();
        const bytes = te.encode("A中B");
        const text = td.decode(bytes);

        const b64 = btoa("ABC");
        const raw = atob(b64);

        return JSON.stringify({
          text,
          b64,
          raw,
          byteLen: bytes.length
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["text"], "A中B");
    assert_eq!(parsed["b64"], "QUJD");
    assert_eq!(parsed["raw"], "ABC");
    assert!(parsed["byteLen"].as_u64().unwrap_or(0) >= 3);
}

#[test]
fn runtime_url_and_search_params() {
    let script = r#"
      (async () => {
        const url = new URL("/v1/items?q=1", "https://example.com/api/");
        url.searchParams.append("q", "2");
        url.searchParams.set("lang", "zh-CN");

        const sp = new URLSearchParams("a=1&a=2");
        const all = sp.getAll("a");
        sp.delete("a");
        sp.append("b", "3");

        return JSON.stringify({
          href: url.href,
          host: url.host,
          q2: url.searchParams.getAll("q").length,
          lang: url.searchParams.get("lang"),
          allLen: all.length,
          sp: sp.toString()
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["host"], "example.com");
    assert_eq!(parsed["q2"], 2);
    assert_eq!(parsed["lang"], "zh-CN");
    assert_eq!(parsed["allLen"], 2);
    assert_eq!(parsed["sp"], "b=3");
    assert!(parsed["href"].as_str().unwrap_or("").contains("lang=zh-CN"));
}

#[test]
fn runtime_process_and_immediate() {
    let script = r#"
      (async () => {
        const events = [];

        process.nextTick(() => events.push("nextTick"));
        const immediateId = setImmediate(() => events.push("immediate"));
        clearImmediate(immediateId);
        setImmediate(() => events.push("immediate-ok"));

        const t0 = process.hrtime();
        const dt = process.hrtime(t0);
        const ns = process.hrtime.bigint();

        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();

        return JSON.stringify({
          hasProcess: typeof process === "object",
          platform: process.platform,
          argvLen: process.argv.length,
          cwd: process.cwd(),
          hasNextTick: events.includes("nextTick"),
          hasImmediateOk: events.includes("immediate-ok"),
          hasImmediateCancelled: events.includes("immediate"),
          hrtimeOk: Array.isArray(dt) && dt.length === 2,
          bigintOk: typeof ns === "bigint"
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["hasProcess"], true);
    assert_eq!(parsed["platform"], "quickjs");
    assert_eq!(parsed["cwd"], "/");
    assert_eq!(parsed["hasNextTick"], true);
    assert_eq!(parsed["hasImmediateOk"], true);
    assert_eq!(parsed["hasImmediateCancelled"], false);
    assert_eq!(parsed["hrtimeOk"], true);
    assert_eq!(parsed["bigintOk"], true);
    assert!(parsed["argvLen"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn runtime_path_module_basic() {
    let script = r#"
      (async () => {
        const path = require("path");
        return JSON.stringify({
          join: path.join("/a", "b", "..", "c.txt"),
          resolve: path.resolve("a", "./b", "../c"),
          dirname: path.dirname("/a/b/c.txt"),
          basename: path.basename("/a/b/c.txt"),
          basenameNoExt: path.basename("/a/b/c.txt", ".txt"),
          ext: path.extname("/a/b/c.txt"),
          abs: path.isAbsolute("/a/b")
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["join"], "/a/c.txt");
    assert_eq!(parsed["resolve"], "/a/c");
    assert_eq!(parsed["dirname"], "/a/b");
    assert_eq!(parsed["basename"], "c.txt");
    assert_eq!(parsed["basenameNoExt"], "c");
    assert_eq!(parsed["ext"], ".txt");
    assert_eq!(parsed["abs"], true);
}

#[test]
fn runtime_buffer_basic() {
    let script = r#"
      (async () => {
        const { Buffer } = require("buffer");
        const a = Buffer.from("ab");
        const b = Buffer.from([99, 100]);
        const c = Buffer.concat([a, b]);
        const d = Buffer.alloc(4, 1);

        return JSON.stringify({
          isBuffer: Buffer.isBuffer(a),
          text: c.toString("utf8"),
          len: Buffer.byteLength("中", "utf8"),
          d: Array.from(d),
          globalOk: typeof globalThis.Buffer === "function"
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["isBuffer"], true);
    assert_eq!(parsed["text"], "abcd");
    assert_eq!(parsed["len"], 3);
    assert_eq!(parsed["d"][0], 1);
    assert_eq!(parsed["d"][1], 1);
    assert_eq!(parsed["d"][2], 1);
    assert_eq!(parsed["d"][3], 1);
    assert_eq!(parsed["globalOk"], true);
}

#[test]
fn runtime_cache_basic_and_concurrent() {
    let script = r#"
      (async () => {
        cache.clear();
        cache.set("num", 1);
        const n1 = cache.get("num");
        const insertedA = cache.setIfAbsent("lock", { v: 1 });
        const insertedB = cache.setIfAbsent("lock", { v: 2 });
        const casFail = cache.compareAndSet("lock", { v: 2 }, { v: 3 });
        const casOk = cache.compareAndSet("lock", { v: 1 }, { v: 3 });
        const lockV = cache.get("lock");
        const hasNum = cache.has("num");
        const deleted = cache.delete("num");
        const n2 = cache.get("num", -1);

        const total = 100;
        await Promise.all(Array.from({ length: total }, (_, i) =>
          Promise.resolve().then(() => cache.set(`k-${i}`, { i }))
        ));
        const values = await Promise.all(Array.from({ length: total }, (_, i) =>
          Promise.resolve().then(() => cache.get(`k-${i}`))
        ));
        const ok = values.every((v, i) => v && v.i === i);

        return JSON.stringify({
          n1,
          insertedA,
          insertedB,
          casFail,
          casOk,
          lockV,
          hasNum,
          deleted,
          n2,
          count: values.length,
          ok
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["n1"], 1);
    assert_eq!(parsed["insertedA"], true);
    assert_eq!(parsed["insertedB"], false);
    assert_eq!(parsed["casFail"], false);
    assert_eq!(parsed["casOk"], true);
    assert_eq!(parsed["lockV"]["v"], 3);
    assert_eq!(parsed["hasNum"], true);
    assert_eq!(parsed["deleted"], true);
    assert_eq!(parsed["n2"], -1);
    assert_eq!(parsed["count"], 100);
    assert_eq!(parsed["ok"], true);
}

#[test]
fn runtime_stats_exposed() {
    let script = r#"
      (async () => {
        const raw = globalThis.__runtime_stats();
        const s = JSON.parse(raw);
        return JSON.stringify({
          ok: s.ok === true,
          hasPending: typeof s.pending === "object",
          hasLimits: typeof s.limits === "object",
          hasPermits: typeof s.permits === "object",
          hasStale: typeof s.staleDrops === "object",
          hasWasi: typeof s.wasi === "object",
          hasCap: typeof s.wasi.cacheCapacity === "number"
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["hasPending"], true);
    assert_eq!(parsed["hasLimits"], true);
    assert_eq!(parsed["hasPermits"], true);
    assert_eq!(parsed["hasStale"], true);
    assert_eq!(parsed["hasWasi"], true);
    assert_eq!(parsed["hasCap"], true);
}

#[test]
fn runtime_console_hook_emits_to_host_logger() {
    let script = r#"
      (async () => {
        const before = JSON.parse(globalThis.__runtime_stats()).logs || {};
        const beforeEnqueued = Number(before.enqueued || 0);

        console.log("hello", { a: 1 });
        console.warn("warn-msg");
        console.error("err-msg");

        await new Promise((r) => setTimeout(r, 0));
        await new Promise((r) => setTimeout(r, 0));

        const after = JSON.parse(globalThis.__runtime_stats()).logs || {};
        const deltaEnqueued = Number(after.enqueued || 0) - beforeEnqueued;

        return JSON.stringify({
          hasConsole: typeof console === "object" && typeof console.log === "function",
          deltaEnqueued,
          written: Number(after.written || 0),
          dropped: Number(after.dropped || 0)
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["hasConsole"], true);
    assert!(parsed["deltaEnqueued"].as_i64().unwrap_or(0) >= 3);
}

#[test]
fn runtime_runs_compiled_pnpm_bundle() {
    let mut bundle_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    bundle_path.push("pnpm_demo");
    bundle_path.push("dist");
    bundle_path.push("bundle.js");

    let bundle =
        fs::read_to_string(&bundle_path).expect("读取编译产物失败，请先执行 pnpm_demo/pnpm build");
    let bundle_json = serde_json::to_string(&bundle).expect("序列化 bundle 失败");

    let script = format!(
        r#"
      (async () => {{
        const code = {bundle_json};
        eval(code);
        const result = await globalThis.__demoMain();
        return JSON.stringify(result);
      }})()
    "#
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["joined"], "/demo/out.png");
    assert_eq!(parsed["out"][0], 1);
    assert_eq!(parsed["out"][1], 2);
    assert_eq!(parsed["out"][2], 3);
    assert_eq!(parsed["out"][3], 4);
    assert_eq!(parsed["wasi"]["exitCode"], 0);
}

#[test]
fn rust_can_call_js_plugin_info_function() {
    let runtime = Runtime::new().expect("创建 runtime 失败");
    let context = Context::full(&runtime).expect("创建 context 失败");

    context
        .with(|ctx| {
            ctx.eval::<(), _>(WEB_POLYFILL)?;
            ctx.eval::<(), _>(
                r#"
                globalThis.__plugin_get_info = async () => ({
                  name: "image-tools",
                  version: "1.2.3",
                  apiVersion: 1
                });

                globalThis.__plugin_echo = (name, version) => ({ name, version });
                "#,
            )?;

            let info = call_js_global_function(&ctx, "__plugin_get_info".to_string(), None)
                .expect("调用 __plugin_get_info 失败");
            let echoed = call_js_global_function(
                &ctx,
                "__plugin_echo".to_string(),
                Some("[\"demo\",\"0.0.1\"]".to_string()),
            )
            .expect("调用 __plugin_echo 失败");

            assert_eq!(info["name"], "image-tools");
            assert_eq!(info["version"], "1.2.3");
            assert_eq!(info["apiVersion"], 1);
            assert_eq!(echoed["name"], "demo");
            assert_eq!(echoed["version"], "0.0.1");

            Ok::<(), rquickjs::Error>(())
        })
        .expect("执行 context.with 失败");
}

#[test]
fn plugin_helpers_load_and_get_info() {
    let runtime = Runtime::new().expect("创建 runtime 失败");
    let context = Context::full(&runtime).expect("创建 context 失败");

    context
        .with(|ctx| {
            ctx.eval::<(), _>(WEB_POLYFILL)?;

            plugin_load(
                &ctx,
                r#"
                plugin.register({
                  name: "demo-plugin",
                  version: "0.1.0",
                  apiVersion: 1,
                  description: "demo"
                });
                "#
                .to_string(),
            )
            .expect("加载插件脚本失败");

            let info = plugin_get_info(&ctx, "demo-plugin".to_string()).expect("读取插件信息失败");
            let list = plugin_list(&ctx).expect("读取插件列表失败");

            assert_eq!(info["name"], "demo-plugin");
            assert_eq!(info["version"], "0.1.0");
            assert_eq!(info["apiVersion"], 1);
            assert_eq!(list[0]["name"], "demo-plugin");

            Ok::<(), rquickjs::Error>(())
        })
        .expect("执行 context.with 失败");
}
