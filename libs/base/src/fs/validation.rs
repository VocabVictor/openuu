use super::*;

#[inline]
pub(super) fn get_ext(name: &str) -> &str {
    if let Some(i) = name.rfind('.') {
        return &name[i + 1..];
    }
    ""
}

#[inline]
pub(super) fn is_compressed_file(name: &str) -> bool {
    let compressed_exts = ["xz", "gz", "zip", "7z", "rar", "bz2", "tgz", "png", "jpg"];
    let ext = get_ext(name);
    compressed_exts.iter().any(|candidate| ext.eq_ignore_ascii_case(candidate))
}

pub fn validate_file_name_no_traversal(name: &str) -> ResultType<()> {
    if name.bytes().any(|b| b == 0) {
        bail!("file name contains null bytes");
    }
    let has_traversal = name
        .split(|c: char| c == '/' || (cfg!(windows) && c == '\\'))
        .filter(|s| !s.is_empty())
        .any(|s| s == "..");
    if has_traversal {
        bail!("path traversal detected in file name");
    }
    #[cfg(windows)]
    {
        if name.len() >= 2 {
            let bytes = name.as_bytes();
            if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                bail!("absolute path detected in file name");
            }
        }
        if name.starts_with('/') || name.starts_with('\\') {
            bail!("absolute path detected in file name");
        }
    }
    #[cfg(not(windows))]
    if name.starts_with('/') {
        bail!("absolute path detected in file name");
    }
    Ok(())
}

pub(super) fn validate_transfer_file_names(files: &[FileEntry]) -> ResultType<()> {
    // Single-file transfer may use an empty relative name, because
    // the destination file path is carried by transfer metadata.
    if files.len() == 1 && files.first().map_or(false, |f| f.name.is_empty()) {
        return Ok(());
    }
    for file in files {
        if file.name.is_empty() {
            bail!("empty file name in multi-file transfer");
        }
        validate_file_name_no_traversal(&file.name)?;
    }
    Ok(())
}

#[inline]
pub(super) fn validate_fs_path_argument(path: &str, arg_name: &str) -> ResultType<()> {
    if path.is_empty() {
        bail!("{arg_name} cannot be empty");
    }
    if path.bytes().any(|b| b == 0) {
        bail!("{arg_name} contains null bytes");
    }
    Ok(())
}

pub(super) fn validate_no_symlink_components(base: &PathBuf, name: &str) -> ResultType<()> {
    if name.is_empty() {
        return Ok(());
    }
    let mut current = base.clone();
    for component in Path::new(name).components() {
        match component {
            std::path::Component::Normal(seg) => {
                current.push(seg);
                // Best-effort guard: path-based checks are inherently TOCTOU-prone
                // if local filesystem state changes between validation and write.
                match std::fs::symlink_metadata(&current) {
                    Ok(meta) => {
                        // This is inherent to filesystem-based checks and acknowledged as a limitation.
                        // For true protection, you'd need openat(2) / O_NOFOLLOW at write time.
                        if meta.file_type().is_symlink() {
                            bail!("symlink path component is not allowed");
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        // Component does not exist yet, continue best-effort validation.
                    }
                    Err(err) => {
                        bail!(
                            "failed to validate path component '{}': {}",
                            current.display(),
                            err
                        );
                    }
                }
            }
            std::path::Component::CurDir => {}
            _ => {
                bail!("invalid file name component");
            }
        }
    }
    Ok(())
}

/// Validate an untrusted relative file name and existing path components before joining it.
pub fn join_validated_path(base: &PathBuf, name: &str) -> ResultType<PathBuf> {
    validate_file_name_no_traversal(name)?;
    validate_no_symlink_components(base, name)?;
    Ok(TransferJob::join(base, name))
}
