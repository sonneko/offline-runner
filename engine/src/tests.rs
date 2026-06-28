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
        use crate::commands::{echo, pwd, cd, cat, head, tail, cp};
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

        // Test head/tail
        echo(vec!["1\n2\n3\n4\n5".to_string(), ">".to_string(), "lines.txt".to_string()]);
        assert_eq!(head(vec!["-n".to_string(), "2".to_string(), "lines.txt".to_string()]), "1\n2");
        assert_eq!(tail(vec!["-n".to_string(), "2".to_string(), "lines.txt".to_string()]), "4\n5");

        // Test cp
        cp("lines.txt", "lines_copy.txt");
        assert_eq!(cat(vec!["lines_copy.txt".to_string()]), "1\n2\n3\n4\n5");

        // Test stat
        use crate::commands::stat;
        assert!(stat("lines.txt").contains("Size: 9"));
    }

    #[test]
    async fn test_mss_expr() {
        use crate::run_mss;
        assert_eq!(run_mss("$a = \"foo\"\n$b = \"bar\"\nif $a == $a { print(\"yes\") }").await, "yes");
        assert_eq!(run_mss("$a = \"1\"\n$b = \"2\"\n$c = $a + $b\nif $c == \"3\" { print(\"yes\") }").await, "yes");
        assert_eq!(run_mss("$a = \"hello\"\n$b = \"world\"\n$c = $a + \" \" + $b\nif $c == \"hello world\" { print(\"yes\") }").await, "yes");
    }

    #[test]
    async fn test_mss_builtins() {
        use crate::run_mss;
        assert_eq!(run_mss("print(\"hello\", \"world\")").await, "hello world");
        assert_eq!(run_mss("len(\"abc\")").await, "3");
        assert_eq!(run_mss("$a = \"10\"\n$b = \"2\"\nprint($a * $b)").await, "20");
        assert_eq!(run_mss("$a = \"10\"\n$b = \"2\"\nprint($a / $b)").await, "5");
    }

    #[test]
    async fn test_mss_loops() {
        use crate::run_mss;
        let code = "
            $count = \"0\"
            while $count != \"3\" {
                $count = $count + \"1\"
            }
            if $count == \"3\" { print(\"done\") }
        ";
        assert_eq!(run_mss(code).await, "done");

        let code_for = "
            for $i in \"a b c\" {
                print($i)
            }
        ";
        let res = run_mss(code_for).await;
        assert!(res.contains("a"));
        assert!(res.contains("b"));
        assert!(res.contains("c"));
    }

    #[test]
    async fn test_mss_command_sub() {
        use crate::run_mss;
        let code = "
            $res = `echo \"hello\"`
            if $res == \"hello\" { print(\"yes\") }
        ";
        assert_eq!(run_mss(code).await, "yes");
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

        // Test /dev/ emulation
        assert_eq!(vfs.read_file_sync("/dev/null", 0, 100).unwrap(), Vec::<u8>::new());
        assert_eq!(vfs.read_file_sync("/dev/zero", 0, 10).unwrap(), vec![0; 10]);
        assert_eq!(vfs.write_file_sync("/dev/null", b"data", 0).unwrap(), 4);
    }
}
