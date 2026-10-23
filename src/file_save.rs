//! Optimistic working-tree saves. A complete sibling temporary file is flushed
//! before replacement; the original file is never truncated in place.
//! Revalidation detects external edits, not a transactional filesystem CAS:
//! another process can still race the final comparison and replacement.
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use crate::{
    file_document::Save,
    git::Cancel,
    project_files::{self, Project},
};

#[derive(Debug)]
pub enum Error {
    Project(project_files::Error),
    Io(io::Error),
    Conflict,
    ReadOnly,
}
impl From<project_files::Error> for Error {
    fn from(e: project_files::Error) -> Self {
        Self::Project(e)
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn save(project: &Project, path: &Path, request: &Save, cancel: &Cancel) -> Result<(), Error> {
    save_before_replace(project, path, request, cancel, || {})
}

/// Publish a complete new file without replacing a concurrently created target.
/// Empty parent directories may remain after cancellation or failure.
pub(crate) fn create_from_file(
    project: &Project,
    path: &Path,
    source: &Path,
    cancel: &Cancel,
) -> Result<(), Error> {
    let destination = project.resolve_missing(path, cancel)?;
    let parent = path.parent().ok_or(project_files::Error::InvalidPath)?;
    let mut relative = PathBuf::new();
    for component in parent.components() {
        relative.push(component);
        let directory = project.resolve_missing(&relative, cancel)?;
        match fs::create_dir(&directory) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error.into()),
        }
        let directory = project.resolve(&relative, cancel)?;
        if !fs::metadata(directory)?.is_dir() {
            return Err(Error::Conflict);
        }
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let temporary = Temporary(destination.with_file_name(format!(".plico-get-{suffix}.tmp")));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.0)?;
    copy_stream(source, &mut file, cancel)?;
    file.sync_all()?;
    drop(file);
    if project.resolve_missing(path, cancel)? != destination {
        return Err(Error::Conflict);
    }
    // Unlike rename, hard_link never overwrites a destination that appeared
    // after preparation. The temporary name is removed by its owner on return.
    #[cfg(not(windows))]
    fs::hard_link(&temporary.0, &destination)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let from: Vec<u16> = temporary
            .0
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // No REPLACE_EXISTING: a concurrent creator wins, including on filesystems
        // without hard links. The complete sibling is moved in one OS operation.
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(), 0)
        } == 0
        {
            return Err(io::Error::last_os_error().into());
        }
    }
    Ok(())
}

fn save_before_replace(
    project: &Project,
    path: &Path,
    request: &Save,
    cancel: &Cancel,
    before_replace: impl FnOnce(),
) -> Result<(), Error> {
    if request.bytes.len() > project_files::FILE_LIMIT {
        return Err(project_files::Error::FileTooLarge.into());
    }
    let destination = project.resolve(path, cancel)?;
    let original = project.read_bytes(path, cancel)?;
    if original.as_slice() != request.expected.as_ref() {
        return Err(Error::Conflict);
    }
    let permissions = fs::metadata(&destination)?.permissions();
    if permissions.readonly() {
        return Err(Error::ReadOnly);
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let temporary_path = destination.with_file_name(format!(".plico-save-{suffix}.tmp"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)?;
    let temporary = Temporary(temporary_path);
    file.set_permissions(permissions)?;
    file.write_all(&request.bytes)?;
    file.sync_all()?;
    drop(file);

    before_replace();
    // Resolve ancestors again and read bytes again, including equal-size changes
    // hidden by filesystem timestamp granularity. Never recreate a deleted file.
    if project.resolve(path, cancel)? != destination
        || project.read_bytes(path, cancel)?.as_slice() != request.expected.as_ref()
    {
        return Err(Error::Conflict);
    }
    if fs::metadata(&destination)?.permissions().readonly() {
        return Err(Error::ReadOnly);
    }
    if cancel.cancelled() {
        return Err(project_files::Error::Cancelled.into());
    }
    replace(&temporary.0, &destination)?;
    // Once replacement succeeds, acknowledge success even if cancellation was
    // requested concurrently; the new disk baseline must not be forgotten.
    Ok(())
}

/// Bounded-memory transport for external merge files. No editor size limit.
pub(crate) fn copy_stream(
    source: &Path,
    output: &mut fs::File,
    cancel: &Cancel,
) -> Result<(), Error> {
    let mut source = fs::File::open(source)?;
    if !source.metadata()?.is_file() {
        return Err(Error::Conflict);
    }
    let mut buffer = [0; 64 * 1024];
    loop {
        if cancel.cancelled() {
            return Err(project_files::Error::Cancelled.into());
        }
        let count = source.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        output.write_all(&buffer[..count])?;
    }
}

pub(crate) fn same_file_bytes(left: &Path, right: &Path, cancel: &Cancel) -> Result<bool, Error> {
    let mut left = fs::File::open(left)?;
    let mut right = fs::File::open(right)?;
    if !left.metadata()?.is_file() || !right.metadata()?.is_file() {
        return Err(Error::Conflict);
    }
    let length = left.metadata()?.len();
    if length != right.metadata()?.len() {
        return Ok(false);
    }
    let mut a = [0; 64 * 1024];
    let mut b = [0; 64 * 1024];
    let mut remaining = length;
    while remaining != 0 {
        if cancel.cancelled() {
            return Err(project_files::Error::Cancelled.into());
        }
        let count = remaining.min(a.len() as u64) as usize;
        left.read_exact(&mut a[..count])?;
        right.read_exact(&mut b[..count])?;
        if a[..count] != b[..count] {
            return Ok(false);
        }
        remaining -= count as u64;
    }
    // Detect growth during the comparison as well as differing initial lengths.
    Ok(left.read(&mut a[..1])? == 0 && right.read(&mut b[..1])? == 0)
}

pub(crate) fn save_from_file(
    project: &Project,
    path: &Path,
    expected: &Path,
    source: &Path,
    cancel: &Cancel,
) -> Result<(), Error> {
    let destination = project.resolve(path, cancel)?;
    if !same_file_bytes(&destination, expected, cancel)? {
        return Err(Error::Conflict);
    }
    let permissions = fs::metadata(&destination)?.permissions();
    if permissions.readonly() {
        return Err(Error::ReadOnly);
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let temporary_path = destination.with_file_name(format!(".plico-save-{suffix}.tmp"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)?;
    let temporary = Temporary(temporary_path);
    file.set_permissions(permissions)?;
    copy_stream(source, &mut file, cancel)?;
    file.sync_all()?;
    drop(file);
    if project.resolve(path, cancel)? != destination
        || !same_file_bytes(&destination, expected, cancel)?
    {
        return Err(Error::Conflict);
    }
    if fs::metadata(&destination)?.permissions().readonly() {
        return Err(Error::ReadOnly);
    }
    if cancel.cancelled() {
        return Err(project_files::Error::Cancelled.into());
    }
    replace(&temporary.0, &destination)?;
    Ok(())
}

#[cfg(not(windows))]
fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path = destination
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    let unc = path
        .strip_prefix("\\\\?\\unc\\")
        .or_else(|| path.strip_prefix("\\\\"));
    let is_wsl = unc.is_some_and(|p| p.starts_with("wsl.localhost\\") || p.starts_with("wsl$\\"));
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Preserve destination security attributes through the Windows replacement
    // API. Do not fall back to deleting the destination on sharing/ACL failure.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            destination.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok != 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    // The WSL redirector rejects ReplaceFileW (ERROR_NOT_SUPPORTED). Rename
    // the fully written sibling instead; never delete/truncate the destination.
    // Do not hide sharing, permission, or ordinary Windows filesystem failures.
    if is_wsl && error.raw_os_error() == Some(50) {
        let moved = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING,
            )
        };
        if moved != 0 {
            return Ok(());
        }
        return Err(io::Error::last_os_error());
    }
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_document::Document;
    struct Fixture {
        path: PathBuf,
        project: Project,
    }
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
            let mut doc = Document::new(
                self.project
                    .read_bytes(Path::new("file.txt"), &Cancel::default())
                    .unwrap(),
            )
            .unwrap();
            doc.edit("after\n".into()).unwrap();
            let request = doc.begin_save().unwrap().unwrap();
            (doc, request)
        }
        fn no_temporaries(&self) {
            assert!(fs::read_dir(&self.path).unwrap().all(|e| {
                !e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".plico-save-")
            }));
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn saves_complete_file_and_preserves_format() {
        let f = Fixture::new();
        let (mut doc, request) = f.request();
        save(
            &f.project,
            Path::new("file.txt"),
            &request,
            &Cancel::default(),
        )
        .unwrap();
        assert_eq!(
            fs::read(f.path.join("file.txt")).unwrap(),
            b"\xef\xbb\xbfafter\r\n"
        );
        doc.finish_save(request, true);
        assert!(!doc.dirty());
        f.no_temporaries();
    }

    #[test]
    fn detects_external_edit_even_after_temp_is_written() {
        let f = Fixture::new();
        let (mut doc, request) = f.request();
        let result = save_before_replace(
            &f.project,
            Path::new("file.txt"),
            &request,
            &Cancel::default(),
            || {
                fs::write(f.path.join("file.txt"), b"\xef\xbb\xbfextern\r\n").unwrap();
            },
        );
        assert!(matches!(result, Err(Error::Conflict)));
        doc.finish_save(request, false);
        assert!(doc.dirty());
        assert_eq!(doc.text(), "after\n");
        assert_eq!(
            fs::read(f.path.join("file.txt")).unwrap(),
            b"\xef\xbb\xbfextern\r\n"
        );
        f.no_temporaries();
    }

    #[test]
    fn cancellation_before_replacement_preserves_original() {
        let f = Fixture::new();
        let (_, request) = f.request();
        let cancel = Cancel::default();
        assert!(
            save_before_replace(&f.project, Path::new("file.txt"), &request, &cancel, || {
                cancel.cancel()
            })
            .is_err()
        );
        assert_eq!(
            fs::read(f.path.join("file.txt")).unwrap().as_slice(),
            request.expected.as_ref()
        );
        f.no_temporaries();
    }

    #[test]
    fn deletion_during_save_is_not_recreated() {
        let f = Fixture::new();
        let (_, request) = f.request();
        assert!(
            save_before_replace(
                &f.project,
                Path::new("file.txt"),
                &request,
                &Cancel::default(),
                || {
                    fs::remove_file(f.path.join("file.txt")).unwrap();
                }
            )
            .is_err()
        );
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
        let supports_readonly = fs::metadata(&path).unwrap().permissions().readonly();
        let result = save(
            &f.project,
            Path::new("file.txt"),
            &request,
            &Cancel::default(),
        );
        fs::set_permissions(&path, original).unwrap();
        // WSL's Windows redirector may accept set_readonly without exposing it
        // in subsequent attributes. Exercise refusal where the flag is supported.
        if supports_readonly {
            assert!(matches!(result, Err(Error::ReadOnly)));
        } else {
            assert!(result.is_ok());
        }
        fs::create_dir(f.path.join(".git")).unwrap();
        fs::write(f.path.join(".git/config"), &request.expected).unwrap();
        assert!(
            save(
                &f.project,
                Path::new(".git/config"),
                &request,
                &Cancel::default()
            )
            .is_err()
        );
        f.no_temporaries();
    }
}
