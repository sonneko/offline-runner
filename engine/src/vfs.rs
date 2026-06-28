use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions};

pub struct Vfs {
    memory_files: HashMap<String, Vec<u8>>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) opfs_root: Option<FileSystemDirectoryHandle>,
    opfs_files: Vec<String>,
    cwd: String,
    pub env_vars: HashMap<String, String>,
}

impl Vfs {
    pub fn new() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("HOME".to_string(), "/".to_string());
        env_vars.insert("PATH".to_string(), "/bin".to_string());

        Self {
            memory_files: HashMap::new(),
            #[cfg(target_arch = "wasm32")]
            opfs_root: None,
            opfs_files: Vec::new(),
            cwd: "/".to_string(),
            env_vars,
        }
    }

    pub fn is_opfs_path(&self, path: &str) -> bool {
        !path.starts_with("/tmp/") && !path.starts_with("/dev/")
    }

    pub fn normalize_path(path: &str) -> String {
        let mut components = Vec::new();
        let is_absolute = path.starts_with('/');

        for component in path.split('/') {
            match component {
                "" | "." => continue,
                ".." => {
                    components.pop();
                }
                _ => components.push(component),
            }
        }

        let mut result = components.join("/");
        if is_absolute {
            result = format!("/{}", result);
        }
        if result.is_empty() {
            if is_absolute { "/" .to_string() } else { "." .to_string() }
        } else {
            result
        }
    }

    pub fn resolve_path(&self, path: &str) -> String {
        if path.starts_with('/') {
            Self::normalize_path(path)
        } else {
            let combined = if self.cwd.ends_with('/') {
                format!("{}{}", self.cwd, path)
            } else {
                format!("{}/{}", self.cwd, path)
            };
            Self::normalize_path(&combined)
        }
    }

    pub fn get_cwd(&self) -> &str {
        &self.cwd
    }

    pub fn get_file_size(&self, path: &str) -> u64 {
        let path = Self::normalize_path(path);
        if path == "/dev/null" { return 0; }
        if path == "/dev/zero" || path == "/dev/random" { return u64::MAX; }

        if !self.is_opfs_path(&path) {
            return self.memory_files.get(&path).map(|v| v.len() as u64).unwrap_or(0);
        }
        0
    }

    pub fn set_cwd(&mut self, path: &str) {
        let resolved = self.resolve_path(path);
        // In a real VFS we should check if it's a directory
        self.cwd = resolved;
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

    pub async fn mkdir_p(path: &str) -> Result<(), JsValue> {
        let path = Self::normalize_path(path);

        #[cfg(target_arch = "wasm32")]
        {
            let root = {
                let vfs = get_vfs().lock().unwrap();
                if !vfs.is_opfs_path(&path) {
                    return Ok(());
                }
                vfs.opfs_root.clone().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?
            };
            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut current_dir = root;

            for component in components {
                let options = web_sys::FileSystemGetDirectoryOptions::new();
                options.set_create(true);
                let next_dir_promise = current_dir.get_directory_handle_with_options(component, &options);
                current_dir = JsFuture::from(next_dir_promise).await?.unchecked_into();
            }
            Ok(())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = path;
            Ok(())
        }
    }

    pub async fn ensure_handle_static(path: &str, create: bool) -> Result<(), JsValue> {
        #[cfg(target_arch = "wasm32")]
        {
            let path = Self::normalize_path(path);
            let root = {
                let vfs = get_vfs().lock().unwrap();
                if !vfs.is_opfs_path(&path) {
                    return Ok(());
                }
                vfs.opfs_root.clone().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?
            };

            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let file_name = components.last().ok_or_else(|| JsValue::from_str("Invalid path"))?;
            let mut current_dir = root;

            for i in 0..components.len()-1 {
                let options = web_sys::FileSystemGetDirectoryOptions::new();
                options.set_create(create);
                let next_dir_promise = current_dir.get_directory_handle_with_options(components[i], &options);
                current_dir = JsFuture::from(next_dir_promise).await?.unchecked_into();
            }

            let options = FileSystemGetFileOptions::new();
            options.set_create(create);
            let _file_handle_promise = current_dir.get_file_handle_with_options(file_name, &options);
            let _file_handle: FileSystemFileHandle = JsFuture::from(_file_handle_promise).await?.unchecked_into();

            {
                let mut vfs = get_vfs().lock().unwrap();
                if !vfs.opfs_files.contains(&path.to_string()) {
                    vfs.opfs_files.push(path.to_string());
                }
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
        if path == "/dev/null" {
            return Ok(Vec::new());
        }
        if path == "/dev/zero" {
            return Ok(vec![0; length]);
        }
        if path == "/dev/random" {
            let mut buf = vec![0; length];
            for i in 0..length {
                buf[i] = (offset.wrapping_add(i as u64) % 256) as u8; // Pseudo-random for now
            }
            return Ok(buf);
        }

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
        if path.starts_with("/dev/") {
            return Ok(content.len());
        }

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

    pub async fn unlink_static(path: &str, recursive: bool) -> Result<(), JsValue> {
        #[cfg(target_arch = "wasm32")]
        {
            let path = Self::normalize_path(path);
            let root = {
                let mut vfs = get_vfs().lock().unwrap();
                if !vfs.is_opfs_path(&path) {
                    vfs.memory_files.remove(&path);
                    return Ok(());
                }
                vfs.opfs_root.clone().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?
            };

            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let file_name = components.last().ok_or_else(|| JsValue::from_str("Invalid path"))?;
            let mut current_dir = root;

            for i in 0..components.len()-1 {
                let next_dir_promise = current_dir.get_directory_handle_with_options(components[i], &web_sys::FileSystemGetDirectoryOptions::new());
                current_dir = JsFuture::from(next_dir_promise).await?.unchecked_into();
            }

            let options = web_sys::FileSystemRemoveOptions::new();
            options.set_recursive(recursive);
            let result = current_dir.remove_entry_with_options(file_name, &options);
            JsFuture::from(result).await?;

            {
                let mut vfs = get_vfs().lock().unwrap();
                vfs.opfs_files.retain(|f| !f.starts_with(&path));
            }
            Ok(())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::normalize_path(path);
            let mut vfs = get_vfs().lock().unwrap();
            vfs.memory_files.remove(&path);
            Ok(())
        }
    }

    pub async fn unlink(&mut self, path: &str) -> Result<(), JsValue> {
        let path = Self::normalize_path(path);
        if !self.is_opfs_path(&path) {
            self.memory_files.remove(&path);
            return Ok(());
        }

        #[cfg(target_arch = "wasm32")]
        {
            let root = self.opfs_root.clone().ok_or_else(|| JsValue::from_str("OPFS not initialized"))?;
            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let file_name = components.last().ok_or_else(|| JsValue::from_str("Invalid path"))?;
            let mut current_dir = root.clone();

            for i in 0..components.len()-1 {
                let next_dir_promise = current_dir.get_directory_handle_with_options(components[i], &web_sys::FileSystemGetDirectoryOptions::new());
                current_dir = JsFuture::from(next_dir_promise).await?.unchecked_into();
            }

            let result = current_dir.remove_entry(file_name);
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
