use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub absolute: PathBuf,
    /// Path segments relative to the walk root, last one being the
    /// filename. Used as the remote destination path.
    pub relative_segments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Dotfile,
    Symlink,
    NotRegularFile,
    NonUtf8,
}

/// Walk a local directory selecting regular files for upload.
///
/// Skips:
/// - dotfiles and dotdirs (anything whose component starts with `.`)
/// - symlinks (no follow)
/// - non-regular files (block/char/fifo/socket)
/// - non-UTF-8 filenames (zz-drop refuses to send them)
///
/// In non-recursive mode, subdirectories are skipped silently.
pub fn walk_local(root: &Path, recursive: bool) -> std::io::Result<Vec<LocalFile>> {
    let mut out = Vec::new();
    walk_inner(root, &mut Vec::new(), recursive, &mut out)?;
    out.sort_by(|a, b| a.relative_segments.cmp(&b.relative_segments));
    Ok(out)
}

fn walk_inner(
    dir: &Path,
    rel_prefix: &mut Vec<String>,
    recursive: bool,
    out: &mut Vec<LocalFile>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue, // non-UTF-8 → skip silently
        };

        if name.starts_with('.') {
            continue;
        }

        // file_type does not follow symlinks
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            if !recursive {
                continue;
            }
            rel_prefix.push(name);
            walk_inner(&path, rel_prefix, recursive, out)?;
            rel_prefix.pop();
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let mut segments = rel_prefix.clone();
        segments.push(name);
        out.push(LocalFile {
            absolute: path,
            relative_segments: segments,
        });
    }
    Ok(())
}

/// Validate that a single user-supplied filename argument is safe for
/// upload, and return its components. The argument may already contain
/// `/` (e.g. `docs/readme.md`), in which case each component is
/// validated.
pub fn split_user_path(arg: &str) -> Result<Vec<String>, SkipReason> {
    if arg.is_empty() {
        return Err(SkipReason::NotRegularFile);
    }
    let mut parts: Vec<String> = Vec::new();
    for part in arg.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(SkipReason::NotRegularFile);
        }
        if part.starts_with('.') {
            return Err(SkipReason::Dotfile);
        }
        parts.push(part.to_string());
    }
    if parts.is_empty() {
        return Err(SkipReason::NotRegularFile);
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn skips_dotfiles() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join(".secret"), b"x").unwrap();
        std::fs::write(tmp.path().join("readme.md"), b"x").unwrap();

        let out = walk_local(tmp.path(), false).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].relative_segments, vec!["readme.md".to_string()]);
    }

    #[test]
    fn skips_symlinks() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join("real.md"), b"x").unwrap();
        symlink(tmp.path().join("real.md"), tmp.path().join("link.md")).unwrap();

        let out = walk_local(tmp.path(), false).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].relative_segments, vec!["real.md".to_string()]);
    }

    #[test]
    fn non_recursive_skips_subdirectories() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join("top.md"), b"x").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("inner.md"), b"x").unwrap();

        let out = walk_local(tmp.path(), false).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].relative_segments, vec!["top.md".to_string()]);
    }

    #[test]
    fn recursive_descends_into_subdirectories() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join("top.md"), b"x").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("inner.md"), b"x").unwrap();
        std::fs::create_dir(tmp.path().join("sub").join("deeper")).unwrap();
        std::fs::write(
            tmp.path().join("sub").join("deeper").join("leaf.md"),
            b"x",
        )
        .unwrap();

        let out = walk_local(tmp.path(), true).unwrap();
        let segs: Vec<Vec<String>> = out.iter().map(|f| f.relative_segments.clone()).collect();
        assert!(segs.contains(&vec!["top.md".to_string()]));
        assert!(segs.contains(&vec!["sub".to_string(), "inner.md".to_string()]));
        assert!(segs.contains(&vec![
            "sub".to_string(),
            "deeper".to_string(),
            "leaf.md".to_string()
        ]));
    }

    #[test]
    fn recursive_still_skips_dotdirs() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        std::fs::write(tmp.path().join(".git").join("HEAD"), b"x").unwrap();
        std::fs::write(tmp.path().join("real.md"), b"x").unwrap();

        let out = walk_local(tmp.path(), true).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].relative_segments, vec!["real.md".to_string()]);
    }

    #[test]
    fn split_user_path_basic() {
        assert_eq!(
            split_user_path("readme.md").unwrap(),
            vec!["readme.md".to_string()]
        );
        assert_eq!(
            split_user_path("docs/readme.md").unwrap(),
            vec!["docs".to_string(), "readme.md".to_string()]
        );
    }

    #[test]
    fn split_user_path_strips_redundant_components() {
        assert_eq!(
            split_user_path("./readme.md").unwrap(),
            vec!["readme.md".to_string()]
        );
    }

    #[test]
    fn split_user_path_rejects_traversal() {
        assert_eq!(
            split_user_path("../etc/passwd"),
            Err(SkipReason::NotRegularFile)
        );
        assert_eq!(
            split_user_path("docs/../etc"),
            Err(SkipReason::NotRegularFile)
        );
    }

    #[test]
    fn split_user_path_rejects_dotfile() {
        assert_eq!(split_user_path(".bashrc"), Err(SkipReason::Dotfile));
    }
}
