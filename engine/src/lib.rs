mod vfs;
mod mss;
mod commands;
#[cfg(test)]
mod tests;

use wasm_bindgen::prelude::*;
use js_sys::Uint8Array;
use std::panic;

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
}

#[wasm_bindgen]
pub fn setup_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        log(&format!("RUST PANIC: {}", msg));
    }));
}

#[wasm_bindgen]
pub async fn execute_command(cmd: &str, args: Vec<String>) -> Result<String, JsValue> {
    // Ensure handles are opened before calling sync commands
    match cmd {
        "grep" | "read" => {
            if let Some(path) = args.get(1) {
                let mut vfs = vfs::get_vfs().lock().unwrap();
                vfs.ensure_handle(path, false).await?;
            }
        }
        "write" | "touch" | "rm" | "stat" => {
            if let Some(path) = args.get(0) {
                let mut vfs = vfs::get_vfs().lock().unwrap();
                vfs.ensure_handle(path, true).await?;
            }
        }
        _ => {}
    }

    let res = match cmd {
        "ls" => commands::ls(args),
        "grep" => {
            if args.len() >= 2 {
                commands::grep(&args[0], &args[1])
            } else {
                "grep requires pattern and path".to_string()
            }
        },
        "find" => {
            if args.len() >= 2 {
                commands::find(&args[0], &args[1])
            } else {
                "find requires path and pattern".to_string()
            }
        },
        "xargs" => {
            if args.len() >= 2 {
                commands::xargs(&args[0], &args[1])
            } else {
                "xargs requires command and input".to_string()
            }
        },
        "write" => {
            if args.len() >= 2 {
                let mut vfs = vfs::get_vfs().lock().unwrap();
                match vfs.write_file_sync(&args[0], args[1].as_bytes(), 0) {
                    Ok(n) => format!("Wrote {} bytes", n),
                    Err(e) => format!("Write Error: {:?}", e),
                }
            } else {
                "write requires path and content".to_string()
            }
        },
        "mkdir" => {
            if args.len() >= 1 {
                let mut vfs = vfs::get_vfs().lock().unwrap();
                match vfs.mkdir_p(&args[0]).await {
                    Ok(_) => format!("Directory created: {}", args[0]),
                    Err(e) => format!("mkdir Error: {:?}", e),
                }
            } else {
                "mkdir requires path".to_string()
            }
        },
        "touch" => {
            if args.len() >= 1 {
                format!("File touched: {}", args[0])
            } else {
                "touch requires path".to_string()
            }
        },
        "rm" => {
            if args.len() >= 1 {
                let mut vfs = vfs::get_vfs().lock().unwrap();
                match vfs.unlink(&args[0]).await {
                    Ok(_) => format!("Removed: {}", args[0]),
                    Err(e) => format!("rm Error: {:?}", e),
                }
            } else {
                "rm requires path".to_string()
            }
        },
        _ => format!("Unknown command: {}", cmd),
    };
    Ok(res)
}

#[wasm_bindgen]
pub fn run_mss(code: &str) -> String {
    let mut interpreter = mss::Interpreter::new();
    interpreter.run(code)
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

    let mut vfs = vfs::get_vfs().lock().unwrap();
    vfs.set_opfs_root(root.unchecked_into());
    vfs.write_file_mem("welcome.txt", b"Welcome to iOS PWA Tool!".to_vec());
    Ok(())
    }
}

impl vfs::Vfs {
    #[cfg(target_arch = "wasm32")]
    pub fn set_opfs_root(&mut self, root: web_sys::FileSystemDirectoryHandle) {
        self.opfs_root = Some(root);
    }
}
