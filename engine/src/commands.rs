use crate::vfs::get_vfs;
use regex::Regex;

pub fn parse_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_content = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_content = true;
            }
            ' ' if !in_quotes => {
                if has_content || !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                    has_content = false;
                }
            }
            _ => {
                current.push(c);
                has_content = true;
            }
        }
    }

    if has_content || !current.is_empty() {
        args.push(current);
    }

    args
}

pub fn ls(args: Vec<String>) -> String {
    let show_all = args.iter().any(|arg| arg == "-a");
    let long_format = args.iter().any(|arg| arg == "-l");

    let vfs = get_vfs().lock().unwrap();
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

    let vfs = get_vfs().lock().unwrap();
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

    let vfs = get_vfs().lock().unwrap();
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

pub fn xargs(_cmd: &str, _input: &str) -> String {
    format!("xargs is currently limited in this environment")
}

pub fn stat(path: &str) -> String {
    let vfs = get_vfs().lock().unwrap();
    let path = vfs.resolve_path(path);
    let size = vfs.get_file_size(&path);

    if vfs.is_opfs_path(&path) {
        format!("File: {}\nSize: {}\nType: OPFS", path, size)
    } else {
        match vfs.read_file_sync(&path, 0, 0) {
            Ok(_) => format!("File: {}\nSize: {}\nType: Memory", path, size),
            Err(_) => format!("stat: {}: No such file", path),
        }
    }
}

pub fn pwd() -> String {
    let vfs = get_vfs().lock().unwrap();
    vfs.get_cwd().to_string()
}

pub fn cd(path: &str) -> String {
    let mut vfs = get_vfs().lock().unwrap();
    vfs.set_cwd(path);
    String::new()
}

pub fn cat(paths: Vec<String>) -> String {
    let vfs = get_vfs().lock().unwrap();
    let mut result = Vec::new();
    let chunk_size = 64 * 1024; // 64KB chunks

    for path in paths {
        let resolved = vfs.resolve_path(&path);
        let mut offset = 0;
        let mut file_content = Vec::new();

        loop {
            match vfs.read_file_sync(&resolved, offset, chunk_size) {
                Ok(content) if !content.is_empty() => {
                    offset += content.len() as u64;
                    file_content.extend_from_slice(&content);
                    // Limit total size to 10MB to avoid excessive memory usage in the PWA
                    if file_content.len() > 10 * 1024 * 1024 {
                        file_content.extend_from_slice(b"\n[File too large, truncated]");
                        break;
                    }
                }
                _ => break,
            }
        }

        if offset == 0 {
            // Check if file exists but is empty or not found
            match vfs.read_file_sync(&resolved, 0, 0) {
                Ok(_) => {}, // File is empty, that's fine
                Err(_) => {
                    result.push(format!("cat: {}: No such file or directory", path));
                    continue;
                }
            }
        }

        result.push(String::from_utf8_lossy(&file_content).to_string());
    }
    result.join("")
}

pub fn echo(args: Vec<String>) -> String {
    let mut redirect_idx = None;
    for (i, arg) in args.iter().enumerate() {
        if arg == ">" {
            redirect_idx = Some(i);
            break;
        }
    }

    if let Some(idx) = redirect_idx {
        let content = args[..idx].join(" ");
        if let Some(path) = args.get(idx + 1) {
            let mut vfs = get_vfs().lock().unwrap();
            let resolved = vfs.resolve_path(path);
            match vfs.write_file_sync(&resolved, content.as_bytes(), 0) {
                Ok(_) => {
                    // Truncate the file to the written length to handle cases where
                    // the new content is shorter than the old one.
                    let _ = vfs.truncate(&resolved, content.len() as u64);
                    String::new()
                },
                Err(e) => format!("echo: write error: {:?}", e),
            }
        } else {
            "echo: missing file for redirection".to_string()
        }
    } else {
        args.join(" ")
    }
}

pub fn head(args: Vec<String>) -> String {
    let n = args.iter().position(|r| r == "-n")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);

    let path = args.last().unwrap_or(&"".to_string()).clone();
    let vfs = get_vfs().lock().unwrap();
    let resolved = vfs.resolve_path(&path);

    match vfs.read_file_sync(&resolved, 0, 64 * 1024) {
        Ok(content) => {
            let text = String::from_utf8_lossy(&content);
            let lines: Vec<&str> = text.lines().take(n).collect();
            lines.join("\n")
        }
        Err(_) => format!("head: {}: No such file", path),
    }
}

pub fn tail(args: Vec<String>) -> String {
    let n = args.iter().position(|r| r == "-n")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);

    let path = args.last().unwrap_or(&"".to_string()).clone();
    let vfs = get_vfs().lock().unwrap();
    let resolved = vfs.resolve_path(&path);

    match vfs.read_file_sync(&resolved, 0, 1024 * 1024) { // Read last 1MB
        Ok(content) => {
            let text = String::from_utf8_lossy(&content);
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(n);
            lines[start..].join("\n")
        }
        Err(_) => format!("tail: {}: No such file", path),
    }
}

pub fn cp(src: &str, dest: &str) -> String {
    let mut vfs = get_vfs().lock().unwrap();
    let src_res = vfs.resolve_path(src);
    let dest_res = vfs.resolve_path(dest);

    let chunk_size = 64 * 1024;
    let mut offset = 0;

    loop {
        match vfs.read_file_sync(&src_res, offset, chunk_size) {
            Ok(content) if !content.is_empty() => {
                if let Err(e) = vfs.write_file_sync(&dest_res, &content, offset) {
                    return format!("cp: write error: {:?}", e);
                }
                offset += content.len() as u64;
            }
            Ok(_) => break,
            Err(e) => return format!("cp: read error: {:?}", e),
        }
    }

    // Ensure destination is truncated to the copied size if it was larger
    let _ = vfs.truncate(&dest_res, offset);
    String::new()
}

pub async fn mv(src: &str, dest: &str) -> String {
    let res = cp(src, dest);
    if res.is_empty() {
        let vfs = get_vfs().lock().unwrap();
        let src_res = vfs.resolve_path(src);
        drop(vfs);
        match crate::vfs::Vfs::unlink_static(&src_res, true).await {
            Ok(_) => String::new(),
            Err(e) => format!("mv: unlink error: {:?}", e),
        }
    } else {
        res
    }
}

pub async fn rm(args: Vec<String>) -> String {
    let recursive = args.iter().any(|arg| arg == "-r");
    let paths: Vec<String> = args.into_iter().filter(|arg| arg != "-r").collect();

    if paths.is_empty() {
        return "rm: missing operand".to_string();
    }

    let mut errors = Vec::new();
    for path in paths {
        let resolved = {
            let vfs = get_vfs().lock().unwrap();
            vfs.resolve_path(&path)
        };
        match crate::vfs::Vfs::unlink_static(&resolved, recursive).await {
            Ok(_) => {},
            Err(e) => errors.push(format!("rm: {}: {:?}", path, e)),
        }
    }

    if errors.is_empty() {
        String::new()
    } else {
        errors.join("\n")
    }
}
