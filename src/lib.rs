use std::{
    sync::{Arc, Mutex},
    thread,
};

use deno_core::{anyhow, serde_v8, v8, JsRuntime};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

pub struct JsExecutor {
    sender: mpsc::Sender<(String, oneshot::Sender<serde_json::Value>)>,
}

impl JsExecutor {
    // new js executor pool
    pub fn new(buffer_size: usize) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<(String, oneshot::Sender<Value>)>(buffer_size);
        let js_executor = Arc::new(JsExecutor { sender });
        let receiver = Arc::new(Mutex::new(Some(receiver)));

        let receiver = Arc::clone(&receiver);
        thread::spawn(move || {
            let mut js_runtime = JsRuntime::new(Default::default());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            runtime.block_on(async move {
                loop {
                    let task_opt = {
                        let mut receiver_guard = receiver.lock().unwrap();
                        receiver_guard.as_mut().unwrap().recv().await
                    };

                    if let Some((code, resp_tx)) = task_opt {
                        let res = js_runtime
                            .execute_script("runjs", code.to_owned())
                            .and_then(|object| {
                                let scope = &mut js_runtime.handle_scope();
                                let local = v8::Local::new(scope, object);
                                serde_v8::from_v8::<Value>(scope, local).map_err(|e| e.into())
                            })
                            .map_err(
                                |e| json!({"code": -1, "msg": format!("ExecutionError: {:?}", e)}),
                            );
                        if let Err(e) = resp_tx.send(res.unwrap_or_else(|err| err)) {
                            eprintln!("Failed to send execution result: {:?}", e);
                        }
                    }
                }
            });
        });

        js_executor
    }

    pub async fn execute<S: AsRef<str>>(
        &self,
        code: S,
    ) -> Result<serde_json::Value, anyhow::Error> {
        let sender = self.sender.clone();
        let code = code.as_ref().to_string();
        let handle = tokio::spawn(async move {
            let (resp_tx, resp_rx) = oneshot::channel();
            if sender.send((code, resp_tx)).await.is_err() {
                return json!({"msg": "Failed to send code for execution"});
            }
            match resp_rx.await {
                Ok(response) => response,
                Err(_) => json!({"msg": "Failed to receive execution result"}),
            }
        });

        handle
            .await
            .map_err(|_| anyhow::Error::msg("handle await error"))
    }
}


#[cfg(test)]
mod test {
    use super::*;
    #[tokio::test]
    async fn call_js() {
        let js_executor = JsExecutor::new(32);
        let res = js_executor.execute("1+1").await;
        println!("===> res: {:?}", res);
    }
}