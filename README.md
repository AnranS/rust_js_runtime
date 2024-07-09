# A light lib light you can run js in rust.
## how to usage
```rust
lazy_static! {
    pub static ref JS_EXECUTOR: Arc<JsExecutor> = JsExecutor::new(32);
}
let res: Result<serde_json::Value, Error> = JS_EXECUTOR.execute("1+1").await;
println!("===> res: {:?}", res);
```