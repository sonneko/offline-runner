mod vfs;
mod mss;
mod commands;
#[cfg(test)]
mod tests;

use wasm_bindgen::prelude::*;
use js_sys::Uint8Array;
use std::panic;
use std::sync::Arc;
use futures::future::LocalBoxFuture;
use log::{Record, Level, Metadata, LevelFilter};

struct WebLogger;

impl log::Log for WebLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            log(&format!("{} - {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {}
}

static LOGGER: WebLogger = WebLogger;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_name = readSync)]
    fn js_read_sync(path: &str, offset: f64, length: f64) -> Uint8Array;

    #[wasm_bindgen(js_name = writeSync)]
    fn js_write_sync(path: &str, content: &[u8], offset: f64) -> f64;

    #[wasm_bindgen(js_name = truncateSync)]
    fn js_truncate_sync(path: &str, size: f64);

    #[wasm_bindgen(js_name = httpGet)]
    async fn js_http_get(url: &str) -> JsValue;

    #[wasm_bindgen(js_name = sleep)]
    async fn js_sleep(ms: f64);
}

#[wasm_bindgen]
pub fn setup_engine() {
    #[cfg(target_arch = "wasm32")]
    {
        panic::set_hook(Box::new(|info| {
            let msg = info.to_string();
            log(&format!("RUST PANIC: {}", msg));
        }));
        log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info)).ok();
    }
}

#[wasm_bindgen]
pub async fn execute_command(cmd_line: &str) -> Result<String, JsValue> {
    let pipes: Vec<&str> = cmd_line.split('|').collect();
    let mut current_input = String::new();
    let mut final_output = String::new();

    for (i, pipe) in pipes.iter().enumerate() {
        let trimmed = pipe.trim();
        let mut args = commands::parse_args(trimmed);

        // If there's input from previous command, append it to args or handle it
        if !current_input.is_empty() {
            args.push(current_input.clone());
        }

        if args.is_empty() { continue; }

        let res = execute_single_command(&args).await?;
        current_input = res.clone();
        if i == pipes.len() - 1 {
            final_output = res;
        }
    }

    Ok(final_output)
}

pub async fn execute_single_command(args: &[String]) -> Result<String, JsValue> {
    if args.is_empty() {
        return Ok(String::new());
    }

    let cmd = &args[0];
    let cmd_args = args[1..].to_vec();

    // Ensure handles are opened before calling sync commands
    {
        let vfs = vfs::get_vfs().lock().unwrap();
        match cmd.as_str() {
            "grep" => {
                if let Some(path) = cmd_args.get(1) {
                    let resolved = vfs.resolve_path(path);
                    drop(vfs);
                    vfs::Vfs::ensure_handle_static(&resolved, false).await?;
                }
            }
            "write" | "touch" => {
                if let Some(path) = cmd_args.get(0) {
                    let resolved = vfs.resolve_path(path);
                    drop(vfs);
                    vfs::Vfs::ensure_handle_static(&resolved, true).await?;
                }
            }
            "rm" | "stat" | "cat" | "head" | "tail" => {
                let paths: Vec<String> = cmd_args.iter()
                    .filter(|p| !p.starts_with('-'))
                    .map(|p| vfs.resolve_path(p))
                    .collect();
                drop(vfs);
                for resolved in paths {
                    vfs::Vfs::ensure_handle_static(&resolved, false).await?;
                }
            }
            "echo" => {
                if let Some(idx) = cmd_args.iter().position(|r| r == ">") {
                    if let Some(path) = cmd_args.get(idx + 1) {
                        let resolved = vfs.resolve_path(path);
                        drop(vfs);
                        vfs::Vfs::ensure_handle_static(&resolved, true).await?;
                    }
                }
            }
            "cp" | "mv" => {
                if cmd_args.len() >= 2 {
                    let src = vfs.resolve_path(&cmd_args[0]);
                    let dest = vfs.resolve_path(&cmd_args[1]);
                    drop(vfs);
                    vfs::Vfs::ensure_handle_static(&src, false).await?;
                    vfs::Vfs::ensure_handle_static(&dest, true).await?;
                }
            }
            _ => {}
        }
    }

    let res = match cmd.as_str() {
        "ls" => commands::ls(cmd_args),
        "pwd" => commands::pwd(),
        "cd" => {
            if let Some(path) = cmd_args.get(0) {
                commands::cd(path)
            } else {
                commands::cd("/")
            }
        },
        "cat" => commands::cat(cmd_args),
        "head" => commands::head(cmd_args),
        "tail" => commands::tail(cmd_args),
        "echo" => commands::echo(cmd_args),
        "grep" => {
            if cmd_args.len() >= 2 {
                let vfs = vfs::get_vfs().lock().unwrap();
                let resolved = vfs.resolve_path(&cmd_args[1]);
                drop(vfs);
                commands::grep(&cmd_args[0], &resolved)
            } else {
                "grep requires pattern and path".to_string()
            }
        },
        "find" => {
            if cmd_args.len() >= 2 {
                let vfs = vfs::get_vfs().lock().unwrap();
                let resolved = vfs.resolve_path(&cmd_args[0]);
                drop(vfs);
                commands::find(&resolved, &cmd_args[1])
            } else {
                "find requires path and pattern".to_string()
            }
        },
        "xargs" => {
            if cmd_args.len() >= 2 {
                commands::xargs(&cmd_args[0], &cmd_args[1])
            } else {
                "xargs requires command and input".to_string()
            }
        },
        "cp" => {
            if cmd_args.len() >= 2 {
                commands::cp(&cmd_args[0], &cmd_args[1])
            } else {
                "cp requires src and dest".to_string()
            }
        },
        "mv" => {
            if cmd_args.len() >= 2 {
                commands::mv(&cmd_args[0], &cmd_args[1]).await
            } else {
                "mv requires src and dest".to_string()
            }
        },
        "write" => {
            if cmd_args.len() >= 2 {
                let mut vfs = vfs::get_vfs().lock().unwrap();
                let resolved = vfs.resolve_path(&cmd_args[0]);
                match vfs.write_file_sync(&resolved, cmd_args[1].as_bytes(), 0) {
                    Ok(n) => format!("Wrote {} bytes", n),
                    Err(e) => format!("Write Error: {:?}", e),
                }
            } else {
                "write requires path and content".to_string()
            }
        },
        "mkdir" => {
            if cmd_args.len() >= 1 {
                let vfs_lock = vfs::get_vfs().lock().unwrap();
                let resolved = vfs_lock.resolve_path(&cmd_args[0]);
                drop(vfs_lock);
                match vfs::Vfs::mkdir_p(&resolved).await {
                    Ok(_) => format!("Directory created: {}", resolved),
                    Err(e) => format!("mkdir Error: {:?}", e),
                }
            } else {
                "mkdir requires path".to_string()
            }
        },
        "touch" => {
            if cmd_args.len() >= 1 {
                format!("File touched: {}", cmd_args[0])
            } else {
                "touch requires path".to_string()
            }
        },
        "rm" => commands::rm(cmd_args).await,
        "stat" => {
            if cmd_args.len() >= 1 {
                commands::stat(&cmd_args[0])
            } else {
                "stat requires path".to_string()
            }
        },
        "print" => cmd_args.join(" "),
        _ => format!("Unknown command: {}", cmd),
    };
    Ok(res)
}

#[wasm_bindgen]
pub fn lint_mss(code: &str) -> Result<(), JsValue> {
    match mss::parse_program(code) {
        Ok((remaining, _)) => {
            if remaining.trim().is_empty() {
                Ok(())
            } else {
                Err(JsValue::from_str(&format!("Parse Error: unexpected trailing characters '{}'", remaining)))
            }
        }
        Err(e) => Err(JsValue::from_str(&format!("Parse Error: {:?}", e))),
    }
}

#[wasm_bindgen]
pub async fn run_mss(code: &str) -> String {
    let executor = |cmd_line: String| -> LocalBoxFuture<'static, Result<String, String>> {
        Box::pin(async move {
            execute_command(&cmd_line).await
                .map_err(|e| format!("{:?}", e))
        })
    };
    let mss_executor = |cmd_line: String| -> LocalBoxFuture<'static, Result<String, String>> {
        Box::pin(async move {
            execute_command(&cmd_line).await
                .map_err(|e| format!("{:?}", e))
        })
    };

    let http_get_fn = |url: String| -> LocalBoxFuture<'static, Result<String, String>> {
        Box::pin(async move {
            let res = js_http_get(&url).await;
            res.as_string().ok_or_else(|| "Failed to get response as string".to_string())
        })
    };

    let sleep_fn = |ms: u64| -> LocalBoxFuture<'static, ()> {
        Box::pin(async move {
            js_sleep(ms as f64).await;
        })
    };

    let mut interpreter = mss::Interpreter::new(mss_executor, http_get_fn, sleep_fn);


    interpreter.set_env("HOME", "/home/user");
    interpreter.set_env("PATH", "/bin:/usr/bin");

    // Static cancel flag for interrupt signal
    if INTERRUPT_FLAG.load(Ordering::SeqCst) {
        INTERRUPT_FLAG.store(false, Ordering::SeqCst);
        return "Interrupted".to_string();
    }
    interpreter.cancel_flag = INTERRUPT_PTR.with(|ptr: &Arc<AtomicBool>| ptr.clone());

    interpreter.run(code).await
}

use std::sync::atomic::{AtomicBool, Ordering};
static INTERRUPT_FLAG: AtomicBool = AtomicBool::new(false);

thread_local! {
    static INTERRUPT_PTR: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

#[wasm_bindgen]
pub fn interrupt() {
    INTERRUPT_FLAG.store(true, Ordering::SeqCst);
    INTERRUPT_PTR.with(|ptr: &Arc<AtomicBool>| ptr.store(true, Ordering::SeqCst));
}

#[wasm_bindgen]
pub fn clear_interrupt() {
    INTERRUPT_FLAG.store(false, Ordering::SeqCst);
}

#[wasm_bindgen]
pub async fn init_vfs() -> Result<(), JsValue> {
    #[cfg(not(target_arch = "wasm32"))]
    { return Ok(()); }

    #[cfg(target_arch = "wasm32")]
    {
    // Avoid holding lock across await
    let root = {
        let global = js_sys::global();
        let storage = if let Ok(worker_scope) = global.clone().dyn_into::<web_sys::WorkerGlobalScope>() {
            let navigator = js_sys::Reflect::get(&worker_scope, &JsValue::from_str("navigator"))?;
            let storage = js_sys::Reflect::get(&navigator, &JsValue::from_str("storage"))?;
            storage.unchecked_into::<web_sys::StorageManager>()
        } else if let Ok(window) = global.dyn_into::<web_sys::Window>() {
            window.navigator().storage()
        } else {
            return Err(JsValue::from_str("Unsupported global scope"));
        };
        let root_promise = storage.get_directory();
        wasm_bindgen_futures::JsFuture::from(root_promise).await?
    };

    {
        let mut vfs = vfs::get_vfs().lock().unwrap();
        vfs.set_opfs_root(root.unchecked_into());
        vfs.write_file_mem("welcome.txt", b"Welcome to iOS PWA Tool!".to_vec());
    }
    Ok(())
    }
}

impl vfs::Vfs {
    #[cfg(target_arch = "wasm32")]
    pub fn set_opfs_root(&mut self, root: web_sys::FileSystemDirectoryHandle) {
        self.opfs_root = Some(root);
    }

}
