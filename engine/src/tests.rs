#[cfg(test)]
mod tests {
    use crate::vfs::Vfs;

    #[test]
    fn test_path_normalization() {
        assert_eq!(Vfs::normalize_path("/a/b/c"), "/a/b/c");
        assert_eq!(Vfs::normalize_path("/a/../b/c"), "/b/c");
        assert_eq!(Vfs::normalize_path("a/./b/../c"), "a/c");
        assert_eq!(Vfs::normalize_path("///a//b"), "/a/b");
        assert_eq!(Vfs::normalize_path("/"), "/");
        assert_eq!(Vfs::normalize_path(""), ".");
    }

    #[test]
    fn test_resolve_path() {
        let mut vfs = Vfs::new();
        assert_eq!(vfs.resolve_path("a/b"), "/a/b");
        vfs.set_cwd("/home");
        assert_eq!(vfs.resolve_path("docs"), "/home/docs");
        assert_eq!(vfs.resolve_path("../etc"), "/etc");
        assert_eq!(vfs.resolve_path("/tmp"), "/tmp");
    }

    #[test]
    fn test_commands() {
        use crate::commands::{echo, pwd, cd, cat};
        use crate::vfs::get_vfs;

        assert_eq!(echo(vec!["hello".to_string(), "world".to_string()]), "hello world");

        {
            let mut vfs = get_vfs().lock().unwrap();
            vfs.set_cwd("/");
        }
        assert_eq!(pwd(), "/");
        cd("/tmp");
        assert_eq!(pwd(), "/tmp");

        echo(vec!["test".to_string(), ">".to_string(), "foo.txt".to_string()]);
        assert_eq!(cat(vec!["foo.txt".to_string()]), "test");
    }

    #[test]
    fn test_parse_args() {
        use crate::commands::parse_args;
        assert_eq!(parse_args("ls -l /tmp"), vec!["ls", "-l", "/tmp"]);
        assert_eq!(parse_args("echo \"hello world\""), vec!["echo", "hello world"]);
        assert_eq!(parse_args("grep \"foo bar\" file.txt"), vec!["grep", "foo bar", "file.txt"]);
        assert_eq!(parse_args("  space  test  "), vec!["space", "test"]);
        assert_eq!(parse_args("cmd \"\""), vec!["cmd", ""]);
    }

    #[test]
    fn test_memory_vfs() {
        let mut vfs = Vfs::new();
        vfs.write_file_mem("/tmp/test.txt", b"hello".to_vec());
        let content = vfs.read_file_sync("/tmp/test.txt", 0, 5).unwrap();
        assert_eq!(content, b"hello");

        vfs.write_file_sync("/tmp/test.txt", b" world", 5).unwrap();
        let content = vfs.read_file_sync("/tmp/test.txt", 0, 11).unwrap();
        assert_eq!(content, b"hello world");
    }
}
