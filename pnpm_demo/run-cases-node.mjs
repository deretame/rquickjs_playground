import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const caseNames = ["fetch", "xhr", "axios", "fs", "native", "runtime"];

for (const name of caseNames) {
  const file = resolve("dist/cases", `${name}.js`);
  const code = await readFile(file, "utf8");
  globalThis.__caseMain = undefined;
  eval(code);
  if (typeof globalThis.__caseMain !== "function") {
    throw new Error(`case ${name} 未导出 __caseMain`);
  }
  const out = await globalThis.__caseMain({});
  if (!out || out.ok !== true) {
    throw new Error(`case ${name} 在 node 运行失败: ${JSON.stringify(out)}`);
  }
}

console.log(JSON.stringify({ ok: true, cases: caseNames.length }));
