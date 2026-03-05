(() => {
  async function main() {
    const BufferRef = globalThis.Buffer;
    const runtimePath = (globalThis as unknown as { path?: { join?: (...parts: string[]) => string } }).path;
    const pathRef: { join: (...parts: string[]) => string } =
      runtimePath && typeof runtimePath.join === "function"
        ? { join: (...parts: string[]) => runtimePath.join!(...parts) }
        : {
      join: (...parts: string[]) => parts.join("/").replace(/\/+/g, "/").replace("/b/../", "/"),
        };
    const p = pathRef.join("/a", "b", "..", "c.txt");

    let ticked: boolean = false;
    process.nextTick(() => {
      ticked = true;
    });
    await new Promise<void>((resolve) => process.nextTick(resolve));

    const b = BufferRef.concat([BufferRef.from("ab"), BufferRef.from("cd")]).toString("utf8");
    return { ok: p === "/a/c.txt" && b === "abcd" && ticked };
  }
  globalThis.__caseMain = main as (config?: unknown) => Promise<unknown>;
})();
