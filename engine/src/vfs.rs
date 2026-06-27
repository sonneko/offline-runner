use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Vfs {
    files: HashMap<String, Vec<u8>>,
}

impl Vfs {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn write_file(&mut self, path: &str, content: Vec<u8>) {
        self.files.insert(path.to_string(), content);
    }

    pub fn read_file(&self, path: &str) -> Option<&Vec<u8>> {
        self.files.get(path)
    }

    pub fn list_files(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    pub fn delete_file(&mut self, path: &str) {
        self.files.remove(path);
    }
}

lazy_static::lazy_static! {
    pub static ref VFS: Arc<Mutex<Vfs>> = Arc::new(Mutex::new(Vfs::new()));
}
