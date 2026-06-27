mod vfs;
mod mss;
mod commands;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn execute_command(cmd: &str, args: Vec<String>) -> Result<String, JsValue> {
    // 1. Identify paths that need handles
    let mut paths_to_ensure = Vec::new();
    if (cmd == "grep" || cmd == "read") && args.len() >= 2 {
        paths_to_ensure.push((args[1].clone(), false));
    } else if (cmd == "write" || cmd == "save") && args.len() >= 1 {
        paths_to_ensure.push((args[0].clone(), true));
    }

    // 2. Ensure handles ARE opened BEFORE locking the Mutex
    for (path, create) in paths_to_ensure {
        // We still need the lock to check if we already have the handle,
        // but ensure_handle itself is async.
        // Actually, let's make a specialized async function that doesn't hold the lock.
        ensure_vfs_handle(&path, create).await?;
    }

    // 3. Lock Mutex only for the synchronous execution
    let res = {
        if cmd == "ls" {
             let mut vfs = vfs::VFS.lock().unwrap();
             vfs.refresh_opfs_listing().await?;
        }
        let _vfs = vfs::VFS.lock().unwrap();
        match cmd {
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
                    let mut vfs_lock = vfs::VFS.lock().unwrap();
                    match vfs_lock.write_file_sync(&args[0], args[1].as_bytes(), 0) {
                        Ok(n) => format!("Wrote {} bytes", n),
                        Err(e) => format!("Write Error: {:?}", e),
                    }
                } else {
                    "write requires path and content".to_string()
                }
            },
            _ => format!("Unknown command: {}", cmd),
        }
    };
    Ok(res)
}

async fn ensure_vfs_handle(path: &str, create: bool) -> Result<(), JsValue> {
    let mut vfs = vfs::VFS.lock().unwrap();
    if vfs.is_opfs_path(path) && !vfs.has_handle(path) {
        // Drop the lock before awaiting
        let root = vfs.opfs_root.clone();
        drop(vfs);

        if let Some(root) = root {
            let options = web_sys::FileSystemGetFileOptions::new();
            options.set_create(create);
            let file_handle: web_sys::FileSystemFileHandle = wasm_bindgen_futures::JsFuture::from(root.get_file_handle_with_options(path, &options)).await?.unchecked_into();
            let access_handle: web_sys::FileSystemSyncAccessHandle = wasm_bindgen_futures::JsFuture::from(file_handle.create_sync_access_handle()).await?.unchecked_into();

            // Re-acquire lock to insert handle
            let mut vfs = vfs::VFS.lock().unwrap();
            vfs.insert_handle(path, access_handle);
        } else {
            return Err(JsValue::from_str("OPFS not initialized"));
        }
    }
    Ok(())
}

#[wasm_bindgen]
pub async fn run_mss(code: &str) -> String {
    let mut interpreter = mss::Interpreter::new();
    interpreter.run(code).await
}

#[wasm_bindgen]
pub async fn init_vfs() -> Result<(), JsValue> {
    let root: web_sys::FileSystemDirectoryHandle = {
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
        wasm_bindgen_futures::JsFuture::from(storage.get_directory()).await?.unchecked_into()
    };

    let mut vfs = vfs::VFS.lock().unwrap();
    vfs.opfs_root = Some(root);
    vfs.write_file_mem("welcome.txt", b"Welcome to iOS PWA Tool!".to_vec());
    Ok(())
}
