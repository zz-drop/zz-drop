use zz_drop_core::providers::nextcloud::collision::rename_with_suffix;
use zz_drop_core::providers::nextcloud::path::{
    PathError, encode_path, encode_remote_root, encode_segment, validate_filename,
};
use zz_drop_core::providers::nextcloud::{RemoteEntry, parse_propfind_multistatus};
use zz_drop_core::providers::nextcloud::webdav::BasicAuth;

// --- validation -------------------------------------------------------

#[test]
fn empty_filename_is_rejected() {
    assert_eq!(validate_filename(""), Err(PathError::Empty));
}

#[test]
fn dot_and_double_dot_rejected() {
    assert_eq!(validate_filename("."), Err(PathError::ParentReference));
    assert_eq!(validate_filename(".."), Err(PathError::ParentReference));
}

#[test]
fn forward_slash_rejected() {
    assert_eq!(validate_filename("a/b"), Err(PathError::HasSeparator));
    assert_eq!(validate_filename("/"), Err(PathError::HasSeparator));
}

#[test]
fn backslash_rejected() {
    assert_eq!(validate_filename("a\\b"), Err(PathError::HasSeparator));
}

#[test]
fn nul_byte_rejected() {
    assert_eq!(validate_filename("a\0b"), Err(PathError::HasNul));
}

#[test]
fn ordinary_filenames_accepted() {
    for name in [
        "readme.md",
        "leggimi.txt",
        "report 2026.pdf",
        "naïve.txt",
        "日本語.txt",
        ".bashrc",
        "foo.tar.gz",
    ] {
        assert!(
            validate_filename(name).is_ok(),
            "expected `{name}` to be valid"
        );
    }
}

// --- encoding ---------------------------------------------------------

#[test]
fn ascii_passthrough_when_safe() {
    assert_eq!(encode_segment("readme.md"), "readme.md");
    assert_eq!(encode_segment("foo.tar.gz"), "foo.tar.gz");
    assert_eq!(encode_segment(".bashrc"), ".bashrc");
}

#[test]
fn space_is_percent_encoded() {
    assert_eq!(encode_segment("report 2026.pdf"), "report%202026.pdf");
}

#[test]
fn special_characters_are_encoded() {
    assert_eq!(encode_segment("a#b"), "a%23b");
    assert_eq!(encode_segment("a?b"), "a%3Fb");
    assert_eq!(encode_segment("a%b"), "a%25b");
}

#[test]
fn unicode_segments_are_percent_encoded_utf8() {
    // Every byte of "日" (U+65E5) gets percent-encoded.
    let out = encode_segment("日本語.txt");
    assert!(out.contains('%'), "expected percent-encoded UTF-8: `{out}`");
    assert!(out.ends_with(".txt"));
}

#[test]
fn encode_path_joins_with_slashes() {
    let p = encode_path(&["docs", "readme.md"]).unwrap();
    assert_eq!(p, "docs/readme.md");
}

#[test]
fn encode_path_rejects_traversal_segment() {
    assert_eq!(
        encode_path(&["docs", "..", "etc"]),
        Err(PathError::ParentReference)
    );
}

#[test]
fn encode_path_empty_segment_rejected() {
    assert_eq!(encode_path(&["docs", ""]), Err(PathError::Empty));
}

#[test]
fn encode_remote_root_handles_variants() {
    assert_eq!(encode_remote_root("").unwrap(), "/");
    assert_eq!(encode_remote_root("/").unwrap(), "/");
    assert_eq!(encode_remote_root("/zz-drop").unwrap(), "/zz-drop");
    assert_eq!(encode_remote_root("/zz-drop/").unwrap(), "/zz-drop");
    assert_eq!(encode_remote_root("/a/b/c").unwrap(), "/a/b/c");
    assert_eq!(
        encode_remote_root("/a b/c"),
        Ok("/a%20b/c".to_string())
    );
}

#[test]
fn encode_remote_root_rejects_traversal() {
    assert_eq!(encode_remote_root("/a/../b"), Err(PathError::ParentReference));
}

// --- collision rename -------------------------------------------------

#[test]
fn rename_zero_is_identity() {
    assert_eq!(rename_with_suffix("foo.md", 0), "foo.md");
    assert_eq!(rename_with_suffix("noext", 0), "noext");
}

#[test]
fn rename_appends_index_before_extension() {
    assert_eq!(rename_with_suffix("foo.md", 1), "foo (1).md");
    assert_eq!(rename_with_suffix("foo.md", 12), "foo (12).md");
}

#[test]
fn rename_no_extension_appends_at_end() {
    assert_eq!(rename_with_suffix("README", 1), "README (1)");
}

#[test]
fn rename_dotfile_treated_as_no_extension() {
    // Per std::path::file_stem, ".bashrc" has stem ".bashrc" and no
    // extension. We follow that convention.
    assert_eq!(rename_with_suffix(".bashrc", 1), ".bashrc (1)");
}

#[test]
fn rename_compound_extension_renames_only_outermost() {
    assert_eq!(rename_with_suffix("foo.tar.gz", 1), "foo.tar (1).gz");
}

// --- basic auth header ------------------------------------------------

#[test]
fn basic_auth_header_format() {
    let a = BasicAuth {
        username: "user".into(),
        password: "p4ss".into(),
    };
    // base64("user:p4ss") = dXNlcjpwNHNz
    assert_eq!(a.header_value(), "Basic dXNlcjpwNHNz");
}

#[test]
fn basic_auth_handles_unicode() {
    let a = BasicAuth {
        username: "naïve".into(),
        password: "ξ".into(),
    };
    let h = a.header_value();
    assert!(h.starts_with("Basic "));
    // Plaintext credentials must not appear in the header value.
    assert!(!h.contains("naïve"));
    assert!(!h.contains('ξ'));
}

// --- PROPFIND multistatus parser --------------------------------------

const SAMPLE_MULTISTATUS: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/user/zz-drop/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/user/zz-drop/readme.md</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>readme.md</d:displayname>
        <d:getcontentlength>2048</d:getcontentlength>
        <d:resourcetype/>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/user/zz-drop/docs/</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>docs</d:displayname>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/user/zz-drop/report%202026.pdf</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>report 2026.pdf</d:displayname>
        <d:getcontentlength>34816</d:getcontentlength>
        <d:resourcetype/>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

#[test]
fn propfind_parser_skips_collection_self_and_returns_children() {
    let entries = parse_propfind_multistatus(SAMPLE_MULTISTATUS).unwrap();
    assert_eq!(entries.len(), 3);

    assert_eq!(
        entries[0],
        RemoteEntry {
            name: "readme.md".to_string(),
            size: Some(2048),
            is_directory: false,
        }
    );
    assert_eq!(
        entries[1],
        RemoteEntry {
            name: "docs".to_string(),
            size: None,
            is_directory: true,
        }
    );
    assert_eq!(
        entries[2],
        RemoteEntry {
            name: "report 2026.pdf".to_string(),
            size: Some(34816),
            is_directory: false,
        }
    );
}

#[test]
fn propfind_parser_handles_empty_multistatus() {
    let xml = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:"></d:multistatus>"#;
    let entries = parse_propfind_multistatus(xml).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn propfind_parser_returns_none_on_garbage() {
    let res = parse_propfind_multistatus("not xml at all <<<>");
    assert!(res.is_none());
}
