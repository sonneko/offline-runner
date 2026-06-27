mod vfs;
mod mss;
mod commands;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn execute_command(cmd: &str, args: Vec<String>) -> String {
    match cmd {
        "ls" => commands::ls(),
        "grep" => {
            if args.len() >= 2 {
                commands::grep(&args[0], &args[1])
            } else {
                "grep requires pattern and path".to_string()
            }
        },
        _ => format!("Unknown command: {}", cmd),
    }
}

#[wasm_bindgen]
pub fn run_mss(code: &str) -> String {
    mss::Interpreter::run(code)
}

#[wasm_bindgen]
pub fn init_vfs() {
    let mut vfs = vfs::VFS.lock().unwrap();
    vfs.write_file("welcome.txt", b"Welcome to iOS PWA Tool!".to_vec());
}
