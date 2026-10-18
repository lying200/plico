//! Optimistic working-tree saves. A complete sibling temporary file is flushed
//! before replacement; the original file is never truncated in place.
//! Revalidation detects external edits, not a transactional filesystem CAS:
//! another process can still race the final comparison and replacement.
use std::{fs, io::{self, Write}, path::{Path, PathBuf}};

use crate::{file_document::Save, git::Cancel, project_files::{self, Project}};

#[derive(Debug)]
pub enum Error {
    Project(project_files::Error),
    Io(io::Error),
    Conflict,
    ReadOnly,
}
impl From<project_files::Error> for Error {
    fn from(e: project_files::Error) -> Self { Self::Project(e) }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self { Self::Io(e) }
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

pub fn save(project: &Project, path: &Path, request: &Save, cancel: &Cancel) -> Result<(), Error> {
    save_before_replace(project, path, request, cancel, || {})
}

fn save_before_replace(
    project: &Project, path: &Path, request: &Save, cancel: &Cancel,
    before_replace: impl FnOnce(),
) -> Result<(), Error> {
    if request.bytes.len() > project_files::FILE_LIMIT {
        return Err(project_files::Error::FileTooLarge.into());
    }
    let destination = project.resolve(path, cancel)?;
    let original = project.read_bytes(path, cancel)?;
    if original.as_slice() != request.expected.as_ref() { return Err(Error::Conflict); }
    let permissions = fs::metadata(&destination)?.permissions();
    if permissions.readonly() { return Err(Error::ReadOnly); }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let temporary_path = destination.with_file_name(format!(".plico-save-{suffix}.tmp"));
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary_path)?;
    let temporary = Temporary(temporary_path);
    file.set_permissions(permissions)?;
    file.write_all(&request.bytes)?;
    file.sync_all()?;
    drop(file);

    before_replace();
    // Resolve ancestors again and read bytes again, including equal-size changes
    // hidden by filesystem timestamp granularity. Never recreate a deleted file.
    if project.resolve(path, cancel)? != destination
        || project.read_bytes(path, cancel)?.as_slice() != request.expected.as_ref() {
        return Err(Error::Conflict);
    }
    if fs::metadata(&destination)?.permissions().readonly() { return Err(Error::ReadOnly); }
    if cancel.cancelled() { return Err(project_files::Error::Cancelled.into()); }
    replace(&temporary.0, &destination)?;
    // Once replacement succeeds, acknowledge success even if cancellation was
    // requested concurrently; the new disk baseline must not be forgotten.
    Ok(())
}

#[cfg(not(windows))]
fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
    // Preserve destination security attributes through the Windows replacement
    // API. Do not fall back to deleting the destination on sharing/ACL failure.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            destination.as_ptr(), source.as_ptr(), std::ptr::null(), 0,
            std::ptr::null(), std::ptr::null(),
        )
    };
    if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_document::Document;
    struct Fixture { path: PathBuf, project: Project }
    impl Fixture {
        fn new() -> Self {
            let mut random = [0u8; 16];
            getrandom::fill(&mut random).unwrap();
            let path = std::env::temp_dir().join(format!("plico-save-{:x?}", random));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("file.txt"), b"\xef\xbb\xbfbefore\r\n").unwrap();
            let project = Project::new(&path).unwrap();
            Self { path, project }
        }
        fn request(&self) -> (Document, Save) {
            let mut doc = Document::new(self.project.read_bytes(Path::new("file.txt"), &Cancel::default()).unwrap()).unwrap();
            doc.edit("after\n".into()).unwrap();
            let request = doc.begin_save().unwrap().unwrap();
            (doc, request)
        }
        fn no_temporaries(&self) {
            assert!(fs::read_dir(&self.path).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".plico-save-")));
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); } }

    #[test]
    fn saves_complete_file_and_preserves_format() {
        let f = Fixture::new();
        let (mut doc, request) = f.request();
        save(&f.project, Path::new("file.txt"), &request, &Cancel::default()).unwrap();
        assert_eq!(fs::read(f.path.join("file.txt")).unwrap(), b"\xef\xbb\xbfafter\r\n");
        doc.finish_save(request, true);
        assert!(!doc.dirty());
        f.no_temporaries();
    }

    #[test]
    fn detects_external_edit_even_after_temp_is_written() {
        let f = Fixture::new();
        let (mut doc, request) = f.request();
        let result = save_before_replace(&f.project, Path::new("file.txt"), &request, &Cancel::default(), || {
            fs::write(f.path.join("file.txt"), b"\xef\xbb\xbfextern\r\n").unwrap();
        });
        assert!(matches!(result, Err(Error::Conflict)));
        doc.finish_save(request, false);
        assert!(doc.dirty());
        assert_eq!(doc.text(), "after\n");
        assert_eq!(fs::read(f.path.join("file.txt")).unwrap(), b"\xef\xbb\xbfextern\r\n");
        f.no_temporaries();
    }

    #[test]
    fn cancellation_before_replacement_preserves_original() {
        let f = Fixture::new();
        let (_, request) = f.request();
        let cancel = Cancel::default();
        assert!(save_before_replace(&f.project, Path::new("file.txt"), &request, &cancel, || cancel.cancel()).is_err());
        assert_eq!(fs::read(f.path.join("file.txt")).unwrap().as_slice(), request.expected.as_ref());
        f.no_temporaries();
    }

    #[test]
    fn deletion_during_save_is_not_recreated() {
        let f = Fixture::new();
        let (_, request) = f.request();
        assert!(save_before_replace(&f.project, Path::new("file.txt"), &request, &Cancel::default(), || {
            fs::remove_file(f.path.join("file.txt")).unwrap();
        }).is_err());
        assert!(!f.path.join("file.txt").exists());
        f.no_temporaries();
    }

    #[test]
    fn refuses_readonly_files_and_git_administration() {
        let f = Fixture::new();
        let (_, request) = f.request();
        let path = f.path.join("file.txt");
        let original = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = save(&f.project, Path::new("file.txt"), &request, &Cancel::default());
        fs::set_permissions(&path, original).unwrap();
        assert!(matches!(result, Err(Error::ReadOnly)));
        fs::create_dir(f.path.join(".git")).unwrap();
        fs::write(f.path.join(".git/config"), &request.expected).unwrap();
        assert!(save(&f.project, Path::new(".git/config"), &request, &Cancel::default()).is_err());
        f.no_temporaries();
    }
}
