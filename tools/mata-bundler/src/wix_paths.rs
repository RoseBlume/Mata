//! WiX 3 uses classic Win32 paths and cannot parse Windows extended-length
//! (`\\?\`) filesystem paths emitted by `canonicalize` or Cargo metadata.
//! Normalize *only* that namespace prefix, preserving UNC network paths.
use std::path::{Path, PathBuf};

pub fn normalize(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        // Strip only a drive-letter path. Other extended namespaces, such as
        // Volume{GUID} and device paths, have no equivalent ordinary path.
        let bytes = rest.as_bytes();
        if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\' {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_extended_drive_prefix() {
        assert_eq!(normalize(Path::new(r"\\?\C:\Users\James\target\installer.wxs")).to_string_lossy(),
                   r"C:\Users\James\target\installer.wxs");
    }
    #[test]
    fn preserves_network_share() {
        assert_eq!(normalize(Path::new(r"\\?\UNC\server\share\file.wxs")).to_string_lossy(),
                   r"\\server\share\file.wxs");
    }
    #[test]
    fn leaves_extended_device_path_unmodified() {
        let path = Path::new(r"\\?\Volume{abc}\file.wxs");
        assert_eq!(normalize(path), path);
    }
    #[test]
    fn leaves_unprefixed_path_unmodified() {
        assert_eq!(normalize(Path::new(r"C:\Projects\file.wxs")), Path::new(r"C:\Projects\file.wxs"));
    }
}
