//! The rest of the computer: every drive and folder on the machine the box runs on, not
//! only the vault. Storage reaches it through the "../" tile at the top of My Drive.
//!
//! This is the whole computer, so it is on only when the box listens on this computer alone,
//! or was started with --host-files. What it keeps to:
//!   * A path is whole and plain ("C:\Users", "/home"). No "..", no device or network
//!     paths (\\.\, \\?\, \\server\share), no alternate streams ("a.txt:hidden").
//!   * Nothing inside the vault's own .castle folder: it holds the box token and the
//!     security record, which this must not be a way around.
//!   * A drive itself, and the folders that hold the vault, cannot be deleted, renamed or
//!     moved from here.
//!   * Delete sends things to the Recycle Bin (the bin on Linux); "delete forever" is its
//!     own call.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::SystemTime,
};

use axum::http::StatusCode;
use serde::Serialize;

use crate::{ApiError, ApiResult, drives::is_within};

/// A listing stops here so one enormous folder cannot stall the box.
const MAX_LISTED: usize = 50_000;
const MAX_PATH_LEN: usize = 4096;
const MAX_NAME_LEN: usize = 255;
#[cfg(windows)]
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
#[cfg(windows)]
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostNode {
    /// The item's whole path, which is also how it is asked for again.
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    /// The folder it is in; empty for a drive.
    pub parent_id: String,
    pub mime: String,
    pub size: u64,
    pub modified: u64,
    /// Hidden by the system (or, on Linux, a name starting with a dot).
    pub hidden: bool,
    /// A link to somewhere else (a symlink, or a Windows junction).
    pub link: bool,
    pub read_only: bool,
}

fn refused(msg: &str) -> ApiError {
    ApiError(StatusCode::FORBIDDEN, msg.into())
}

/// A path as the app sends it, checked and made plain. See the module notes for the rules.
pub fn parse(text: &str) -> ApiResult<PathBuf> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_PATH_LEN || text.chars().any(char::is_control) {
        return Err(ApiError::bad("invalid path"));
    }
    #[cfg(windows)]
    {
        let text = text.replace('/', "\\");
        let bytes = text.as_bytes();
        let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if !drive || (bytes.len() > 2 && bytes[2] != b'\\') {
            return Err(ApiError::bad("a path on this computer starts with a drive, like C:\\"));
        }
        let mut path = format!("{}:\\", (bytes[0] as char).to_ascii_uppercase());
        let parts: Vec<&str> = text[2..].split('\\').filter(|p| !p.is_empty()).collect();
        for part in &parts {
            if *part == "." || *part == ".." || part.chars().any(|c| r#":*?"<>|"#.contains(c)) {
                return Err(ApiError::bad("invalid path"));
            }
        }
        path.push_str(&parts.join("\\"));
        Ok(PathBuf::from(path))
    }
    #[cfg(not(windows))]
    {
        if !text.starts_with('/') {
            return Err(ApiError::bad("a path on this computer starts with /"));
        }
        let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty()).collect();
        if parts.iter().any(|p| *p == "." || *p == "..") {
            return Err(ApiError::bad("invalid path"));
        }
        Ok(PathBuf::from(format!("/{}", parts.join("/"))))
    }
}

/// A name for something new on this computer. Looser than the vault's (a dot at the start
/// is fine here), but nothing that changes what folder it lands in.
pub fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_LEN || name == "." || name == ".." {
        return false;
    }
    if name.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return false;
    }
    if cfg!(windows) {
        if name.ends_with(' ') || name.ends_with('.') || name.chars().any(|c| r#":*?"<>|"#.contains(c)) {
            return false;
        }
        let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
        if crate::WINDOWS_RESERVED.contains(&stem.as_str()) {
            return false;
        }
    }
    true
}

fn text_of(path: &Path) -> String {
    crate::plain_path(path).to_string_lossy().to_string()
}

fn millis(t: SystemTime) -> u64 {
    crate::millis(t)
}

/// A drive or a mount point's top: there is nothing above it to put it in.
fn is_top(path: &Path) -> bool {
    path.parent().is_none()
}

fn hidden(name: &str, meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let _ = name;
        meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 || name.starts_with('.')
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        name.starts_with('.')
    }
}

pub struct Host {
    /// The vault, and its private folder, as the system resolves them.
    vault: PathBuf,
    keep_out: PathBuf,
}

impl Host {
    pub fn new(vault: &Path) -> Self {
        let vault = vault.canonicalize().unwrap_or_else(|_| vault.to_path_buf());
        Self { keep_out: crate::plain_path(&vault.join(crate::META_DIR)), vault: crate::plain_path(&vault) }
    }

    /// The vault's folder on this computer, so "../" from My Drive lands beside it.
    pub fn vault(&self) -> String {
        text_of(&self.vault)
    }

    /// Where a path really leads once links are followed: the path itself when it does not
    /// exist yet, under wherever its nearest existing folder leads.
    fn real(path: &Path) -> PathBuf {
        let mut rest = Vec::new();
        let mut at = path;
        loop {
            if let Ok(real) = at.canonicalize() {
                let mut out = crate::plain_path(&real);
                for part in rest.iter().rev() {
                    out.push(part);
                }
                return out;
            }
            match (at.parent(), at.file_name()) {
                (Some(parent), Some(name)) => {
                    rest.push(name.to_os_string());
                    at = parent;
                }
                _ => return path.to_path_buf(),
            }
        }
    }

    /// Refuses the vault's private folder, by its path or by any link leading into it.
    fn guard(&self, path: &Path) -> ApiResult<()> {
        if is_within(path, &self.keep_out) || is_within(&Self::real(path), &self.keep_out) {
            return Err(refused("Castle's own private folder cannot be opened from here."));
        }
        Ok(())
    }

    /// For delete, rename and move: refuses a drive itself and the folders the vault is in.
    fn guard_structure(&self, path: &Path) -> ApiResult<()> {
        self.guard(path)?;
        if is_top(path) {
            return Err(refused("A drive itself cannot be deleted, renamed or moved."));
        }
        if is_within(&self.vault, path) || is_within(&self.vault, &Self::real(path)) {
            return Err(refused("The Castle vault is in this folder, so it stays where it is."));
        }
        Ok(())
    }

    fn node(&self, path: &Path, meta: &std::fs::Metadata) -> HostNode {
        let link = meta.file_type().is_symlink();
        // A link shows as what it leads to; a broken one as an empty file.
        let target = if link { std::fs::metadata(path).ok() } else { None };
        let shown = target.as_ref().unwrap_or(meta);
        let is_dir = shown.is_dir();
        let name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => text_of(path).trim_end_matches(['\\', '/']).to_string(),
        };
        let name = if name.is_empty() { "/".to_string() } else { name };
        HostNode {
            id: text_of(path),
            kind: if is_dir { "folder" } else { "file" },
            parent_id: path.parent().map(text_of).unwrap_or_default(),
            mime: if is_dir { String::new() } else { mime_guess::from_path(path).first_or_octet_stream().to_string() },
            size: if is_dir { 0 } else { shown.len() },
            modified: shown.modified().map(millis).unwrap_or(0),
            hidden: hidden(&name, meta),
            link,
            read_only: shown.permissions().readonly(),
            name,
        }
    }

    pub fn stat(&self, path: &Path) -> ApiResult<HostNode> {
        self.guard(path)?;
        Ok(self.node(path, &std::fs::symlink_metadata(path)?))
    }

    /// What is in a folder. An entry the system will not describe is left out, not fatal.
    pub fn list(&self, path: &Path) -> ApiResult<Vec<HostNode>> {
        self.guard(path)?;
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path)?.flatten() {
            if out.len() >= MAX_LISTED {
                break;
            }
            let child = entry.path();
            if entry.file_name().to_str().is_none() || is_within(&child, &self.keep_out) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            out.push(self.node(&child, &meta));
        }
        Ok(out)
    }

    /// A file to send, after the checks. Links are followed: it is the user's own computer.
    pub fn readable_file(&self, path: &Path) -> ApiResult<PathBuf> {
        self.guard(path)?;
        if !std::fs::metadata(path)?.is_file() {
            return Err(ApiError::not_found());
        }
        Ok(path.to_path_buf())
    }

    /// The folder something new goes into, after the checks.
    pub fn folder_for_new(&self, parent: &Path, name: &str) -> ApiResult<PathBuf> {
        if !valid_name(name) {
            return Err(ApiError::bad("invalid name"));
        }
        self.guard(&parent.join(name))?;
        if !std::fs::metadata(parent)?.is_dir() {
            return Err(ApiError::not_found());
        }
        Ok(parent.to_path_buf())
    }

    pub fn mkdir(&self, parent: &Path, name: &str) -> ApiResult<HostNode> {
        let dir = self.folder_for_new(parent, name)?;
        let name = crate::free_name(&dir, name);
        std::fs::create_dir(dir.join(&name))?;
        self.stat(&dir.join(name))
    }

    pub fn rename(&self, path: &Path, name: &str) -> ApiResult<HostNode> {
        if !valid_name(name) {
            return Err(ApiError::bad("invalid name"));
        }
        self.guard_structure(path)?;
        std::fs::symlink_metadata(path)?;
        let parent = path.parent().ok_or_else(|| ApiError::bad("invalid path"))?;
        let dst = parent.join(name);
        self.guard(&dst)?;
        let old = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if old == name {
            return self.stat(path);
        }
        // Windows names ignore case, so "a.txt" to "A.txt" finds itself already there.
        let case_only = cfg!(windows) && old.eq_ignore_ascii_case(name);
        if !case_only && std::fs::symlink_metadata(&dst).is_ok() {
            return Err(ApiError::conflict("that name is already taken"));
        }
        std::fs::rename(path, &dst)?;
        self.stat(&dst)
    }

    /// Moves an item into another folder, on the same drive or another one.
    pub fn move_to(&self, path: &Path, target: &Path) -> ApiResult<HostNode> {
        self.guard_structure(path)?;
        self.guard(target)?;
        if !std::fs::metadata(target)?.is_dir() {
            return Err(ApiError::not_found());
        }
        if is_within(target, path) {
            return Err(ApiError::bad("a folder cannot be moved into itself"));
        }
        let name = path.file_name().ok_or_else(|| ApiError::bad("invalid path"))?;
        let dst = target.join(name);
        if std::fs::symlink_metadata(&dst).is_ok() {
            return Err(ApiError::conflict("that name is already taken"));
        }
        match std::fs::rename(path, &dst) {
            Ok(()) => {}
            // To another drive: copy it whole, and remove the original only once it is there.
            Err(e) if e.kind() == ErrorKind::CrossesDevices => {
                if let Err(e) = copy_tree(path, &dst) {
                    let _ = remove(&dst);
                    return Err(e.into());
                }
                remove(path)?;
            }
            Err(e) => return Err(e.into()),
        }
        self.stat(&dst)
    }

    /// To the Recycle Bin, from where it can be put back with the system's own tools.
    pub fn recycle(&self, path: &Path) -> ApiResult<()> {
        self.guard_structure(path)?;
        std::fs::symlink_metadata(path)?;
        trash::delete(path).map_err(|e| {
            eprintln!("[castled] recycle failed: {e}");
            ApiError::conflict("This could not go to the Recycle Bin. It may be in use, or on a drive with no bin; use Delete forever.")
        })
    }

    pub fn delete(&self, path: &Path) -> ApiResult<()> {
        self.guard_structure(path)?;
        remove(path)?;
        Ok(())
    }
}

/// Removes a file, a folder and everything in it, or a link (never what the link leads to).
fn remove(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        // A link to a folder is removed as a folder on Windows, as a file elsewhere.
        if cfg!(windows) && std::fs::metadata(path).is_ok_and(|m| m.is_dir()) {
            std::fs::remove_dir(path)
        } else {
            std::fs::remove_file(path)
        }
    } else if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// Copies a file or a whole folder. Links inside are not followed, and are left out.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if meta.is_dir() {
        std::fs::create_dir(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() {
                continue;
            }
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// True when the path has nothing like "..", so two spellings cannot mean different places.
#[cfg(test)]
fn is_plain(path: &Path) -> bool {
    use std::path::Component;
    path.components().all(|c| !matches!(c, Component::ParentDir | Component::CurDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("castled-host-{}", crate::random_hex(6)));
        std::fs::create_dir_all(dir.join("vault").join(crate::META_DIR)).unwrap();
        std::fs::write(dir.join("vault").join(crate::META_DIR).join("token"), b"secret").unwrap();
        dir
    }

    #[test]
    fn only_whole_plain_paths_are_accepted() {
        let bad: &[&str] = if cfg!(windows) {
            &["", "relative\\x", "\\\\server\\share", "\\\\?\\C:\\x", "\\\\.\\PhysicalDrive0", "C:\\a\\..\\b", "C:\\a:stream", "C:x", "1:\\"]
        } else {
            &["", "relative/x", "/a/../b", "/a/./b"]
        };
        for text in bad {
            assert!(parse(text).is_err(), "{text:?} should be refused");
        }
        if cfg!(windows) {
            assert_eq!(parse("c:/Users//Brian/").unwrap(), PathBuf::from("C:\\Users\\Brian"));
            assert_eq!(parse("D:").unwrap(), PathBuf::from("D:\\"));
        } else {
            assert_eq!(parse("/home//x/").unwrap(), PathBuf::from("/home/x"));
            assert_eq!(parse("/").unwrap(), PathBuf::from("/"));
        }
        assert!(is_plain(&parse(if cfg!(windows) { "C:\\a\\b" } else { "/a/b" }).unwrap()));
    }

    #[test]
    fn new_names_cannot_change_folder() {
        for bad in ["", ".", "..", "a/b", "a\\b", "x\0y"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        for good in [".env", "notes.txt", "New folder", "résumé.pdf"] {
            assert!(valid_name(good), "{good:?}");
        }
    }

    #[test]
    fn the_vaults_private_folder_stays_out_of_reach() {
        let dir = scratch();
        let host = Host::new(&dir.join("vault"));
        let vault = dir.join("vault");
        // Not listed...
        let names: Vec<String> = host.list(&vault).unwrap().into_iter().map(|n| n.name).collect();
        assert!(!names.contains(&crate::META_DIR.to_string()), "{names:?}");
        // ...and not opened, read, or made into, by its path.
        let private = vault.join(crate::META_DIR);
        assert!(host.list(&private).is_err());
        assert!(host.readable_file(&private.join("token")).is_err());
        assert!(host.mkdir(&private, "x").is_err());
        assert!(host.delete(&private).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_vault_and_drives_cannot_be_pulled_out_from_under_the_box() {
        let dir = scratch();
        let host = Host::new(&dir.join("vault"));
        assert!(host.delete(&dir.join("vault")).is_err());
        assert!(host.delete(&dir).is_err(), "the folder holding the vault");
        assert!(host.rename(&dir.join("vault"), "elsewhere").is_err());
        let top = if cfg!(windows) { PathBuf::from("C:\\") } else { PathBuf::from("/") };
        assert!(host.delete(&top).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn files_are_made_renamed_moved_and_deleted() {
        let dir = scratch();
        let host = Host::new(&dir.join("vault"));
        let made = host.mkdir(&dir, "Work").unwrap();
        assert_eq!(made.kind, "folder");
        assert_eq!(host.mkdir(&dir, "Work").unwrap().name, "Work (2)", "nothing is overwritten");
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
        let renamed = host.rename(&dir.join("a.txt"), "b.txt").unwrap();
        assert_eq!(renamed.name, "b.txt");
        assert!(host.rename(&dir.join("b.txt"), "Work").is_err(), "a taken name");
        let moved = host.move_to(&dir.join("b.txt"), &dir.join("Work")).unwrap();
        assert_eq!(PathBuf::from(&moved.parent_id), crate::plain_path(&dir.join("Work")));
        assert!(host.move_to(&dir.join("Work"), &dir.join("Work")).is_err(), "into itself");
        let listed = host.list(&dir.join("Work")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].size, 5);
        host.delete(&dir.join("Work")).unwrap();
        assert!(!dir.join("Work").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_case_only_rename_works_where_names_ignore_case() {
        let dir = scratch();
        let host = Host::new(&dir.join("vault"));
        std::fs::write(dir.join("readme.txt"), b"x").unwrap();
        assert_eq!(host.rename(&dir.join("readme.txt"), "README.txt").unwrap().name, "README.txt");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn copying_a_folder_copies_everything_in_it() {
        let dir = scratch();
        std::fs::create_dir_all(dir.join("src").join("deep")).unwrap();
        std::fs::write(dir.join("src").join("deep").join("f.bin"), b"123").unwrap();
        copy_tree(&dir.join("src"), &dir.join("dst")).unwrap();
        assert_eq!(std::fs::read(dir.join("dst").join("deep").join("f.bin")).unwrap(), b"123");
        let _ = std::fs::remove_dir_all(dir);
    }
}
