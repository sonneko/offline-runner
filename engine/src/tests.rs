#[cfg(test)]
mod tests {
    use crate::vfs::Vfs;

    #[test]
    fn test_path_normalization() {
        assert_eq!(Vfs::normalize_path("/a/b/c"), "/a/b/c");
        assert_eq!(Vfs::normalize_path("/a/../b/c"), "/b/c");
        assert_eq!(Vfs::normalize_path("a/./b/../c"), "a/c");
        assert_eq!(Vfs::normalize_path("///a//b"), "/a/b");
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
