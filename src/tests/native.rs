use crate::tests::run_async_script;
use serde_json::Value;

#[test]
fn native_run_invert_min_copy_api() {
    let script = r#"
      (async () => {
        const out = await native.run("invert", new Uint8Array([0, 10, 255]));
        return JSON.stringify({
          supportsBinaryBridge: native.supportsBinaryBridge,
          len: out.length,
          v0: out[0],
          v1: out[1],
          v2: out[2]
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["supportsBinaryBridge"], true);
    assert_eq!(parsed["len"], 3);
    assert_eq!(parsed["v0"], 255);
    assert_eq!(parsed["v1"], 245);
    assert_eq!(parsed["v2"], 0);
}

#[test]
fn native_handle_chain_grayscale() {
    let script = r#"
      (async () => {
        const rgba = new Uint8Array([
          255, 0, 0, 255,
          0, 255, 0, 255
        ]);
        const id = await native.put(rgba);
        const grayId = await native.exec("grayscale_rgba", id);
        const out = await native.take(grayId);
        await native.free(grayId);
        return JSON.stringify({
          len: out.length,
          a0: out[0],
          a1: out[1],
          a2: out[2],
          alpha0: out[3],
          b0: out[4],
          b1: out[5],
          b2: out[6],
          alpha1: out[7]
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["len"], 8);
    assert_eq!(parsed["a0"], parsed["a1"]);
    assert_eq!(parsed["a1"], parsed["a2"]);
    assert_eq!(parsed["b0"], parsed["b1"]);
    assert_eq!(parsed["b1"], parsed["b2"]);
    assert_eq!(parsed["alpha0"], 255);
    assert_eq!(parsed["alpha1"], 255);
}

#[test]
fn native_exec_with_extra_input() {
    let script = r#"
      (async () => {
        const left = await native.put(new Uint8Array([1, 2, 3]));
        const right = await native.put(new Uint8Array([3, 2, 1]));
        const outId = await native.exec("xor", left, null, right);
        const out = await native.take(outId);
        await native.free(outId);
        return JSON.stringify({
          v0: out[0],
          v1: out[1],
          v2: out[2]
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["v0"], 2);
    assert_eq!(parsed["v1"], 0);
    assert_eq!(parsed["v2"], 2);
}

#[test]
fn wasi_run_minimal_module() {
    let script = r#"
      (async () => {
        const wasm = new Uint8Array([
          0x00,0x61,0x73,0x6d,0x01,0x00,0x00,0x00,
          0x01,0x04,0x01,0x60,0x00,0x00,
          0x03,0x02,0x01,0x00,
          0x07,0x0a,0x01,0x06,0x5f,0x73,0x74,0x61,0x72,0x74,0x00,0x00,
          0x0a,0x04,0x01,0x02,0x00,0x0b
        ]);
        const result = await wasi.run(wasm);
        const stdout = await wasi.takeStdout(result);
        const stderr = await wasi.takeStderr(result);
        return JSON.stringify({
          exitCode: result.exitCode,
          stdoutLen: stdout.length,
          stderrLen: stderr.length
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["exitCode"], 0);
    assert_eq!(parsed["stdoutLen"], 0);
    assert_eq!(parsed["stderrLen"], 0);
}

#[test]
fn wasi_run_reuse_module_id() {
    let script = r#"
      (async () => {
        const wasm = new Uint8Array([
          0x00,0x61,0x73,0x6d,0x01,0x00,0x00,0x00,
          0x01,0x04,0x01,0x60,0x00,0x00,
          0x03,0x02,0x01,0x00,
          0x07,0x0a,0x01,0x06,0x5f,0x73,0x74,0x61,0x72,0x74,0x00,0x00,
          0x0a,0x04,0x01,0x02,0x00,0x0b
        ]);
        const id = await native.put(wasm);
        const r1 = await wasi.runById(id, { reuseModule: true });
        const r2 = await wasi.runById(id, { reuseModule: true });
        await wasi.takeStdout(r1);
        await wasi.takeStderr(r1);
        await wasi.takeStdout(r2);
        await wasi.takeStderr(r2);
        await native.free(id);
        return JSON.stringify({
          c1: r1.exitCode,
          c2: r2.exitCode
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["c1"], 0);
    assert_eq!(parsed["c2"], 0);
}

#[test]
fn native_take_into_and_chain() {
    let script = r#"
      (async () => {
        const inputId = await native.put(new Uint8Array([1, 2, 3, 4]));
        const outId = await native.execChain(inputId, [
          { op: "invert" },
          { op: "invert" },
          { op: "noop" }
        ]);

        const target = new Uint8Array(8);
        const info = await native.takeInto(outId, target, 2);

        const chained = await native.chain(["invert", "invert"], new Uint8Array([9, 8]));

        return JSON.stringify({
          bytesWritten: info.bytesWritten,
          truncated: info.truncated,
          target: Array.from(target),
          chained: Array.from(chained)
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["bytesWritten"], 4);
    assert_eq!(parsed["truncated"], false);
    assert_eq!(parsed["target"][0], 0);
    assert_eq!(parsed["target"][1], 0);
    assert_eq!(parsed["target"][2], 1);
    assert_eq!(parsed["target"][3], 2);
    assert_eq!(parsed["target"][4], 3);
    assert_eq!(parsed["target"][5], 4);
    assert_eq!(parsed["chained"][0], 9);
    assert_eq!(parsed["chained"][1], 8);
}

#[test]
fn native_take_into_truncated_and_source_length() {
    let script = r#"
      (async () => {
        const id = await native.put(new Uint8Array([10, 11, 12, 13, 14]));
        const target = new Uint8Array(3);
        const info = await native.takeInto(id, target, 1);
        return JSON.stringify({
          bytesWritten: info.bytesWritten,
          sourceLength: info.sourceLength,
          truncated: info.truncated,
          target: Array.from(target)
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["bytesWritten"], 2);
    assert_eq!(parsed["sourceLength"], 5);
    assert_eq!(parsed["truncated"], true);
    assert_eq!(parsed["target"][0], 0);
    assert_eq!(parsed["target"][1], 10);
    assert_eq!(parsed["target"][2], 11);
}

#[test]
fn native_exec_chain_with_extra_input() {
    let script = r#"
      (async () => {
        const left = await native.put(new Uint8Array([1, 2, 3]));
        const right = await native.put(new Uint8Array([3, 2, 1]));
        const outId = await native.execChain(left, [
          { op: "xor", extraInputId: right },
          { op: "invert" }
        ]);
        const out = await native.take(outId);
        return JSON.stringify({ out: Array.from(out) });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["out"][0], 253);
    assert_eq!(parsed["out"][1], 255);
    assert_eq!(parsed["out"][2], 253);
}

#[test]
fn wasi_run_by_id_consumes_module_by_default() {
    let script = r#"
      (async () => {
        const wasm = new Uint8Array([
          0x00,0x61,0x73,0x6d,0x01,0x00,0x00,0x00,
          0x01,0x04,0x01,0x60,0x00,0x00,
          0x03,0x02,0x01,0x00,
          0x07,0x0a,0x01,0x06,0x5f,0x73,0x74,0x61,0x72,0x74,0x00,0x00,
          0x0a,0x04,0x01,0x02,0x00,0x0b
        ]);

        const id = await native.put(wasm);
        const ok = await wasi.runById(id);
        await wasi.takeStdout(ok);
        await wasi.takeStderr(ok);

        let secondError = "";
        try {
          await wasi.runById(id);
        } catch (err) {
          secondError = String(err.message || err);
        }

        return JSON.stringify({
          firstExit: ok.exitCode,
          secondErrorHasId: secondError.includes("module id")
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["firstExit"], 0);
    assert_eq!(parsed["secondErrorHasId"], true);
}

#[test]
fn native_exec_chain_invalid_steps_errors() {
    let script = r#"
      (async () => {
        const inputId = await native.put(new Uint8Array([1, 2, 3]));

        let emptyErr = "";
        try {
          await native.execChain(inputId, []);
        } catch (err) {
          emptyErr = String(err.message || err);
        }

        const inputId2 = await native.put(new Uint8Array([4, 5, 6]));
        let badErr = "";
        try {
          await native.execChain(inputId2, [{}]);
        } catch (err) {
          badErr = String(err.message || err);
        }

        return JSON.stringify({
          emptyHasHint: emptyErr.includes("非空数组") || emptyErr.includes("不能为空"),
          badHasHint: badErr.includes("op")
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["emptyHasHint"], true);
    assert_eq!(parsed["badHasHint"], true);
}

#[test]
fn bridge_call_by_function_name_with_args() {
    let script = r#"
      (async () => {
        const inputId = await bridge.call("native.put", [1, 2, 3]);
        const outId = await bridge.call("native.exec", "invert", inputId, null, null);
        const out = await bridge.call("native.take", outId);
        const sum = await bridge.call("math.add", 1.5, 2);

        return JSON.stringify({
          out,
          sum
        });
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["out"][0], 254);
    assert_eq!(parsed["out"][1], 253);
    assert_eq!(parsed["out"][2], 252);
    assert_eq!(parsed["sum"], 3.5);
}
