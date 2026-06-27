use crate::vfs::VFS;
use regex::Regex;

pub fn ls(args: Vec<String>) -> String {
    let show_all = args.iter().any(|arg| arg == "-a");
    let long_format = args.iter().any(|arg| arg == "-l");

    let vfs = VFS.lock().unwrap();
    let mut files = vfs.list_files();
    files.sort();

    let mut result = Vec::new();
    for file in files {
        if !show_all && file.starts_with('.') {
            continue;
        }

        if long_format {
            result.push(format!("-rw-r--r-- 1 user group {:8} Mar 10 12:00 {}", 0, file));
        } else {
            result.push(file);
        }
    }

    if long_format {
        result.join("\n")
    } else {
        result.join("  ")
    }
}

pub fn grep(pattern: &str, path: &str) -> String {
    let re = match Regex::new(pattern) {
        Ok(re) => re,
        Err(e) => return format!("Invalid regex: {}", e),
    };

    let vfs = VFS.lock().unwrap();
    let chunk_size = 1024 * 64; // 64KB
    let mut offset = 0;
    let mut matched_lines = Vec::new();
    let mut carry_over: Vec<u8> = Vec::new();

    loop {
        match vfs.read_file_sync(path, offset, chunk_size) {
            Ok(buffer) if !buffer.is_empty() => {
                let mut data = carry_over;
                data.extend_from_slice(&buffer);

                let mut start = 0;
                for i in 0..data.len() {
                    if data[i] == b'\n' {
                        let line = String::from_utf8_lossy(&data[start..i]);
                        if re.is_match(&line) {
                            matched_lines.push(line.to_string());
                        }
                        start = i + 1;
                    }
                }
                carry_over = data[start..].to_vec();
                offset += buffer.len() as u64;
            }
            _ => break,
        }
    }

    if !carry_over.is_empty() {
        let line = String::from_utf8_lossy(&carry_over);
        if re.is_match(&line) {
            matched_lines.push(line.to_string());
        }
    }

    if matched_lines.is_empty() {
        "No matches found".to_string()
    } else {
        matched_lines.join("\n")
    }
}

pub fn find(path: &str, pattern: &str) -> String {
    let re = match Regex::new(pattern) {
        Ok(re) => re,
        Err(e) => return format!("Invalid regex: {}", e),
    };

    let vfs = VFS.lock().unwrap();
    let files = vfs.list_files();
    let mut matches = Vec::new();

    for file in files {
        if file.starts_with(path) && re.is_match(&file) {
            matches.push(file);
        }
    }

    if matches.is_empty() {
        "No files found".to_string()
    } else {
        matches.join("\n")
    }
}

pub fn xargs(cmd: &str, input: &str) -> String {
    // This is problematic because execute_command is now async
    // and we are in a sync function.
    // For now, we'll just return an error or keep it for memory files only.
    format!("xargs is currently limited in this environment")
}
