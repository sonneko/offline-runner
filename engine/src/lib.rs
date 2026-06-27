mod vfs;
mod mss;
mod commands;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn execute_command(cmd: &str, args: Vec<String>) -> Result<String, JsValue> {
    // Ensure handles are opened before calling sync commands
    if cmd == "grep" || cmd == "read" {
        if args.len() >= 2 {
            let mut vfs = vfs::VFS.lock().unwrap();
            vfs.ensure_handle(&args[1], false).await?;
        }
    } else if cmd == "write" {
         if args.len() >= 2 {
            let mut vfs = vfs::VFS.lock().unwrap();
            vfs.ensure_handle(&args[0], true).await?;
        }
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
                let mut vfs = vfs::VFS.lock().unwrap();
                match vfs.write_file_sync(&args[0], args[1].as_bytes(), 0) {
                    Ok(n) => format!("Wrote {} bytes", n),
                    Err(e) => format!("Write Error: {:?}", e),
                }
            } else {
                "write requires path and content".to_string()
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

    let mut vfs = vfs::VFS.lock().unwrap();
    vfs.set_opfs_root(root.unchecked_into());
    vfs.write_file_mem("welcome.txt", b"Welcome to iOS PWA Tool!".to_vec());
    Ok(())
}

impl vfs::Vfs {
    pub fn set_opfs_root(&mut self, root: web_sys::FileSystemDirectoryHandle) {
        self.opfs_root = Some(root);
    }
}
