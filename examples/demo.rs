use rquickjs_playground::AsyncHostRuntime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = AsyncHostRuntime::new("example-demo")?;

    let script = r#"
        (async () => {
          plugin.register({
            name: "demo-plugin",
            version: "0.1.0",
            apiVersion: 1
          });

          const inputId = await bridge.call("native.put", [1, 2, 3]);
          const outId = await bridge.call("native.exec", "invert", inputId, null, null);
          const out = await bridge.call("native.take", outId);

          return JSON.stringify({
            plugin: plugin.getInfo("demo-plugin"),
            out,
          });
        })()
    "#;

    let result = host.spawn(script)?.wait()?;
    println!("{result}");
    Ok(())
}
