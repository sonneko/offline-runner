use crate::vfs::VFS;

pub fn ls() -> String {
    let vfs = VFS.lock().unwrap();
    let files = vfs.list_files();
    files.join("\n")
}

pub fn grep(pattern: &str, path: &str) -> String {
    let vfs = VFS.lock().unwrap();
    if let Some(content) = vfs.read_file(path) {
        let text = String::from_utf8_lossy(content);
        text.lines()
            .filter(|line| line.contains(pattern))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        format!("File not found: {}", path)
    }
}
