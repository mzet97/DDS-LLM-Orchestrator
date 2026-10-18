use super::*;

#[test]
fn formatters_match_design() {
    assert_eq!(fmt_gb2(4_680_000_000), "4.68 GB");
    assert_eq!(fmt_gb2(0), "0.00 GB");
    assert_eq!(fmt_hhmmss(9), "00:00:09");
    assert_eq!(fmt_hhmmss(3723), "01:02:03");
    assert_eq!(short_sha("abcdef"), "abcdef");
    assert_eq!(
        short_sha("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        "0123456789abcdef0123456789abcdef…"
    );
}

#[test]
fn host_and_fs_helpers_are_honest() {
    // Caminho inexistente nunca inventa filesystem.
    assert_eq!(
        fs_info(std::path::Path::new("/definitivamente/inexistente-3-7")),
        None
    );
    assert_eq!(
        fs_total_bytes(std::path::Path::new("/definitivamente/inexistente-3-7")),
        None
    );
    #[cfg(target_os = "linux")]
    {
        assert!(!hostname().is_empty(), "hostname real ou —");
        assert!(
            fs_total_bytes(std::path::Path::new("/tmp")).unwrap_or(0) > 0,
            "/tmp tem tamanho"
        );
    }
}
