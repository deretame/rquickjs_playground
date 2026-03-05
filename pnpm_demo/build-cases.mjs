import { build } from "esbuild";

await build({
  entryPoints: [
    "cases/fetch.ts",
    "cases/xhr.ts",
    "cases/axios.ts",
    "cases/fs.ts",
    "cases/native.ts",
    "cases/runtime.ts",
  ],
  bundle: true,
  platform: "browser",
  format: "iife",
  outdir: "dist/cases",
  target: "es2020",
});
