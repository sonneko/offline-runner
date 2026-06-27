use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemSyncAccessHandle, FileSystemGetFileOptions};
use js_sys::Uint8Array;

pub struct Vfs {
    memory_files: HashMap<String, Vec<u8>>,
    pub(crate) opfs_root: Option<FileSystemDirectoryHandle>,
    handle_pool: HashMap<String, FileSystemSyncAccessHandle>,
    opfs_files: Vec<String>,
}

impl Vfs {
    pub fn new() -> Self {
        Self {
            memory_files: HashMap::new(),
            opfs_root: None,
            handle_pool: HashMap::new(),
            opfs_files: Vec::new(),
        }
    }

    pub fn is_opfs_path(&self, path: &str) -> bool {
        !path.starts_with("/tmp/") && !path.starts_with("/dev/")
    }

    pub async fn init_opfs(&mut self) -> Result<(), JsValue> {
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
        let root_js: JsValue = JsFuture::from(root_promise).await?;
        let root: FileSystemDirectoryHandle = root_js.unchecked_into();
        self.opfs_root = Some(root);

        self.refresh_opfs_listing().await?;
        Ok(())
    }

    pub async fn refresh_opfs_listing(&mut self) -> Result<(), JsValue> {
        Ok(())
    }

    pub async fn ensure_handle(&mut self, path: &str, create: bool) -> Result<(), JsValue> {
        if !self.is_opfs_path(path) || self.handle_pool.contains_key(path) {
            return Ok(());
        }

        let root = self.opfs_root.as_ref().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?;
        let options = FileSystemGetFileOptions::new();
        options.set_create(create);

        let file_handle_promise = root.get_file_handle_with_options(path, &options);
        let file_handle: FileSystemFileHandle = JsFuture::from(file_handle_promise).await?.unchecked_into();

        let access_handle_promise = file_handle.create_sync_access_handle();
        let access_handle: FileSystemSyncAccessHandle = JsFuture::from(access_handle_promise).await?.unchecked_into();

        self.handle_pool.insert(path.to_string(), access_handle);
        if !self.opfs_files.contains(&path.to_string()) {
            self.opfs_files.push(path.to_string());
        }
        Ok(())
    }

    pub fn write_file_mem(&mut self, path: &str, content: Vec<u8>) {
        self.memory_files.insert(path.to_string(), content);
    }

    pub fn read_file_sync(&self, path: &str, offset: u64, length: usize) -> Result<Vec<u8>, JsValue> {
        if !self.is_opfs_path(path) {
            return self.memory_files.get(path)
                .map(|v| {
                    let start = offset as usize;
                    let end = (offset as usize + length).min(v.len());
                    if start >= v.len() {
                        Vec::new()
                    } else {
                        v[start..end].to_vec()
                    }
                })
                .ok_or_else(|| JsValue::from_str("File not found in memory"));
        }

        let handle = self.handle_pool.get(path).ok_or_else(|| JsValue::from_str(&format!("Handle not in pool for {}", path)))?;
        let mut buffer = vec![0u8; length];
        let uint8_array = unsafe { Uint8Array::view(&buffer) };

        let options = web_sys::FileSystemReadWriteOptions::new();
        options.set_at(offset as f64);

        let bytes_read = handle.read_with_buffer_source_and_options(&uint8_array, &options)?;
        buffer.truncate(bytes_read as usize);
        Ok(buffer)
    }

    pub fn write_file_sync(&mut self, path: &str, content: &[u8], offset: u64) -> Result<usize, JsValue> {
        if !self.is_opfs_path(path) {
            let entry = self.memory_files.entry(path.to_string()).or_insert_with(Vec::new);
            let end = (offset as usize) + content.len();
            if entry.len() < end {
                entry.resize(end, 0);
            }
            entry[offset as usize..end].copy_from_slice(content);
            return Ok(content.len());
        }

        let handle = self.handle_pool.get(path).ok_or_else(|| JsValue::from_str(&format!("Handle not in pool for {}", path)))?;
        let uint8_array = unsafe { Uint8Array::view(content) };
        let options = web_sys::FileSystemReadWriteOptions::new();
        options.set_at(offset as f64);

        let bytes_written = handle.write_with_buffer_source_and_options(&uint8_array, &options)?;
        Ok(bytes_written as usize)
    }

    pub fn list_files(&self) -> Vec<String> {
        let mut files: Vec<String> = self.memory_files.keys().cloned().collect();
        for path in &self.opfs_files {
            if !files.contains(path) {
                files.push(path.clone());
            }
        }
        files
    }
}

lazy_static::lazy_static! {
    pub static ref VFS: Arc<Mutex<Vfs>> = Arc::new(Mutex::new(Vfs::new()));
}
