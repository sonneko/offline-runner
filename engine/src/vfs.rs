use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemSyncAccessHandle, FileSystemGetFileOptions};
#[cfg(target_arch = "wasm32")]
use js_sys::Uint8Array;

pub struct Vfs {
    memory_files: HashMap<String, Vec<u8>>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) opfs_root: Option<FileSystemDirectoryHandle>,
    opfs_files: Vec<String>,
}

impl Vfs {
    pub fn new() -> Self {
        Self {
            memory_files: HashMap::new(),
            #[cfg(target_arch = "wasm32")]
            opfs_root: None,
            opfs_files: Vec::new(),
        }
    }

    pub fn is_opfs_path(&self, path: &str) -> bool {
        !path.starts_with("/tmp/") && !path.starts_with("/dev/")
    }

    pub fn normalize_path(path: &str) -> String {
        let mut components = Vec::new();
        for component in path.split('/') {
            match component {
                "" | "." => continue,
                ".." => {
                    components.pop();
                }
                _ => components.push(component),
            }
        }
        if path.starts_with('/') {
            format!("/{}", components.join("/"))
        } else {
            components.join("/")
        }
    }

    #[cfg(target_arch = "wasm32")]
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

    pub async fn mkdir_p(&mut self, path: &str) -> Result<(), JsValue> {
        let path = Self::normalize_path(path);
        if !self.is_opfs_path(&path) {
            return Ok(());
        }

        #[cfg(target_arch = "wasm32")]
        {
            let root = self.opfs_root.as_ref().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?;
            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut current_dir = root.clone();

            for component in components {
                let options = web_sys::FileSystemGetDirectoryOptions::new();
                options.set_create(true);
                let next_dir_promise = current_dir.get_directory_handle_with_options(component, &options);
                current_dir = JsFuture::from(next_dir_promise).await?.unchecked_into();
            }
            Ok(())
        }
        #[cfg(not(target_arch = "wasm32"))]
        { Ok(()) }
    }

    pub async fn ensure_handle(&mut self, path: &str, _create: bool) -> Result<(), JsValue> {
        #[cfg(target_arch = "wasm32")]
        {
            let path = Self::normalize_path(path);
            if !self.is_opfs_path(&path) {
                return Ok(());
            }

            let root = self.opfs_root.as_ref().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?;
            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let fileName = components.last().ok_or_else(|| JsValue::from_str("Invalid path"))?;
            let mut current_dir = root.clone();

            for i in 0..components.len()-1 {
                let options = web_sys::FileSystemGetDirectoryOptions::new();
                options.set_create(_create);
                let next_dir_promise = current_dir.get_directory_handle_with_options(components[i], &options);
                current_dir = JsFuture::from(next_dir_promise).await?.unchecked_into();
            }

            let options = FileSystemGetFileOptions::new();
            options.set_create(_create);
            let _file_handle_promise = current_dir.get_file_handle_with_options(fileName, &options);
            let _file_handle: FileSystemFileHandle = JsFuture::from(_file_handle_promise).await?.unchecked_into();

            if !self.opfs_files.contains(&path.to_string()) {
                self.opfs_files.push(path.to_string());
            }
            Ok(())
        }
        #[cfg(not(target_arch = "wasm32"))]
        { Ok(()) }
    }

    pub fn write_file_mem(&mut self, path: &str, content: Vec<u8>) {
        self.memory_files.insert(path.to_string(), content);
    }

    pub fn read_file_sync(&self, path: &str, offset: u64, length: usize) -> Result<Vec<u8>, JsValue> {
        let path = Self::normalize_path(path);
        if !self.is_opfs_path(&path) {
            let res = self.memory_files.get(&path)
                .map(|v| {
                    let start = offset as usize;
                    let end = (offset as usize + length).min(v.len());
                    if start >= v.len() {
                        Vec::new()
                    } else {
                        v[start..end].to_vec()
                    }
                });

            return match res {
                Some(v) => Ok(v),
                None => {
                    #[cfg(target_arch = "wasm32")]
                    { Err(JsValue::from_str("File not found in memory")) }
                    #[cfg(not(target_arch = "wasm32"))]
                    { Err(JsValue::null()) }
                }
            };
        }

        #[cfg(target_arch = "wasm32")]
        {
            let res = crate::js_read_sync(&path, offset as f64, length as f64);
            Ok(res.to_vec())
        }
        #[cfg(not(target_arch = "wasm32"))]
        { Err(JsValue::null()) }
    }

    pub fn write_file_sync(&mut self, path: &str, content: &[u8], offset: u64) -> Result<usize, JsValue> {
        let path = Self::normalize_path(path);
        if !self.is_opfs_path(&path) {
            let entry = self.memory_files.entry(path.to_string()).or_insert_with(Vec::new);
            let end = (offset as usize) + content.len();
            if entry.len() < end {
                entry.resize(end, 0);
            }
            entry[offset as usize..end].copy_from_slice(content);
            return Ok(content.len());
        }

        #[cfg(target_arch = "wasm32")]
        {
            // use Uint8Array::view for zero-copy if possible?
            // js_write_sync takes &[u8], wasm-bindgen handles the copy.
            // To be more efficient, we could use unsafe Uint8Array::view on the Rust side
            // but crate::js_write_sync expects &[u8].
            let written = crate::js_write_sync(&path, content, offset as f64);
            Ok(written as usize)
        }
        #[cfg(not(target_arch = "wasm32"))]
        { Err(JsValue::null()) }
    }

    pub fn truncate(&mut self, path: &str, size: u64) -> Result<(), JsValue> {
        let path = Self::normalize_path(path);
        if !self.is_opfs_path(&path) {
            let entry = self.memory_files.get_mut(&path).ok_or_else(|| {
                 #[cfg(target_arch = "wasm32")]
                 { JsValue::from_str("File not found") }
                 #[cfg(not(target_arch = "wasm32"))]
                 { JsValue::null() }
            })?;
            entry.resize(size as usize, 0);
            return Ok(());
        }

        #[cfg(target_arch = "wasm32")]
        {
            crate::js_truncate_sync(&path, size as f64);
            Ok(())
        }
        #[cfg(not(target_arch = "wasm32"))]
        { Err(JsValue::null()) }
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

    pub async fn unlink(&mut self, path: &str) -> Result<(), JsValue> {
        let path = Self::normalize_path(path);
        if !self.is_opfs_path(&path) {
            self.memory_files.remove(&path);
            return Ok(());
        }

        #[cfg(target_arch = "wasm32")]
        {
            let root = self.opfs_root.as_ref().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?;
            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let fileName = components.last().ok_or_else(|| JsValue::from_str("Invalid path"))?;
            let mut current_dir = root.clone();

            for i in 0..components.len()-1 {
                let next_dir_promise = current_dir.get_directory_handle_with_options(components[i], &web_sys::FileSystemGetDirectoryOptions::new());
                current_dir = JsFuture::from(next_dir_promise).await?.unchecked_into();
            }

            let result = current_dir.remove_entry(fileName);
            JsFuture::from(result).await?;

            self.opfs_files.retain(|f| f != &path);
            Ok(())
        }
        #[cfg(not(target_arch = "wasm32"))]
        { Err(JsValue::null()) }
    }
}

use std::sync::OnceLock;

pub static VFS: OnceLock<Arc<Mutex<Vfs>>> = OnceLock::new();

pub fn get_vfs() -> &'static Arc<Mutex<Vfs>> {
    VFS.get_or_init(|| Arc::new(Mutex::new(Vfs::new())))
}
