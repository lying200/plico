//! Isolated native merge inputs and guarded result write-back.
//! Reference Rebased 4aec352 (JetBrains and contributors, Apache-2.0).
use crate::{
    conflicts::{self, File, Snapshot},
    git::{Cancel, Repository},
    operation_process::{self, Progress},
    project_files::Project,
};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

mod automatic;
mod staging;
pub use automatic::AutomaticResult;
pub(crate) use staging::stage_resolved_with;
pub use staging::{StageResult, StagedFile, checked_report, stage_resolved};

struct Temporary(PathBuf, AtomicBool);
impl Drop for Temporary {
    fn drop(&mut self) {
        if !self.1.load(Ordering::Relaxed) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
pub struct Prepared {
    temporary: Temporary,
    snapshot: Snapshot,
    file: File,
    expected: PathBuf,
    applied: Mutex<Option<PathBuf>>,
    root: PathBuf,
    inputs: [PathBuf; 4],
}
impl Prepared {
    pub fn read(
        repo: &Repository,
        snapshot: &Snapshot,
        file: &File,
        token: &Cancel,
    ) -> Result<Self, String> {
        snapshot.revalidate(repo, file, token)?;
        if file
            .stages
            .iter()
            .flatten()
            .any(|v| v.mode == "160000" || v.mode == "120000")
        {
            return Err(
                "Use Accept Yours/Theirs for symbolic links; resolve submodules externally".into(),
            );
        }
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let name = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let path = std::env::temp_dir().join(format!("plico-merge-{name}"));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&path).map_err(|e| e.to_string())?;
        let temporary = Temporary(path, AtomicBool::new(false));
        let suffix = file
            .path
            .extension()
            .filter(|s| !s.is_empty())
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();
        let inputs = ["yours", "theirs", "base", "result"]
            .map(|name| temporary.0.join(format!("{name}{suffix}")));
        for (index, version) in [
            file.side(conflicts::Side::Yours, snapshot.reversed),
            file.side(conflicts::Side::Theirs, snapshot.reversed),
            file.stages[0].as_ref(),
        ]
        .into_iter()
        .enumerate()
        {
            let output = fs::File::create(&inputs[index]).map_err(|e| e.to_string())?;
            if let Some(version) = version {
                let mut command = repo.operation_command();
                command.args(["cat-file", "blob", &version.oid]);
                let outcome = operation_process::run_to_file(
                    &mut command,
                    token,
                    &Progress::default(),
                    output,
                )
                .map_err(|e| e.to_string())?;
                if !outcome.status.success() || outcome.stopped.is_some() {
                    return Err(format!(
                        "Could not read conflict version: {}",
                        String::from_utf8_lossy(&outcome.stderr.bytes())
                    ));
                }
            }
        }
        let expected = temporary.0.join("working-baseline");
        let original = project
            .resolve(&file.path, token)
            .map_err(|e| format!("{e:?}"))?;
        let mut baseline = fs::File::create(&expected).map_err(|e| e.to_string())?;
        crate::file_save::copy_stream(&original, &mut baseline, token)
            .map_err(|e| format!("{e:?}"))?;
        drop(baseline);
        if project
            .resolve(&file.path, token)
            .map_err(|e| format!("{e:?}"))?
            != original
            || !crate::file_save::same_file_bytes(&original, &expected, token)
                .map_err(|e| format!("{e:?}"))?
        {
            return Err("Working file changed while preparing merge".into());
        }
        let mut output = fs::File::create(&inputs[3]).map_err(|e| e.to_string())?;
        crate::file_save::copy_stream(&expected, &mut output, token)
            .map_err(|e| format!("{e:?}"))?;
        drop(output);
        Ok(Self {
            temporary,
            snapshot: snapshot.clone(),
            file: file.clone(),
            expected,
            applied: Mutex::new(None),
            root: repo.path.clone(),
            inputs,
        })
    }
    /// Bounded inputs for the native viewer. Binary content has a side-selection
    /// viewer; IO, encoding and size failures remain errors, not empty previews.
    pub fn native_text_inputs(&self) -> Result<Option<[String; 3]>, String> {
        let read = |path: &std::path::Path| -> Result<Option<String>, String> {
            let mut bytes = Vec::new();
            fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(crate::project_files::FILE_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            match crate::file_document::Document::new(bytes) {
                Ok(document) => Ok(Some(document.text().to_owned())),
                Err(crate::file_document::Error::Binary) => Ok(None),
                Err(e) => Err(format!("Unsupported merge text: {e:?}")),
            }
        };
        // Read every input even if one is binary: do not hide another IO error.
        let working = read(&self.expected)?;
        let yours = read(&self.inputs[0])?;
        let base = read(&self.inputs[2])?;
        let theirs = read(&self.inputs[1])?;
        Ok(match (working, yours, base, theirs) {
            (Some(_), Some(yours), Some(base), Some(theirs)) => Some([yours, base, theirs]),
            _ => None,
        })
    }
    /// Text-only callers still reject binary input explicitly.
    pub fn text_inputs(&self) -> Result<[String; 3], String> {
        self.native_text_inputs()?
            .ok_or_else(|| "Unsupported merge text: Binary".into())
    }
    /// Encode an explicitly accepted native result using the original file's
    /// separator/BOM. This only touches the isolated output, not the worktree.
    pub fn write_text_result(&self, text: String) -> Result<(), String> {
        let mut bytes = Vec::new();
        fs::File::open(&self.expected)
            .map_err(|e| e.to_string())?
            .take(crate::project_files::FILE_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let mut document =
            crate::file_document::Document::new(bytes).map_err(|e| format!("{e:?}"))?;
        document.edit(text).map_err(|e| format!("{e:?}"))?;
        let bytes = document
            .begin_save()
            .map_err(|e| format!("{e:?}"))?
            .map(|s| s.bytes)
            .unwrap_or_else(|| document.disk());
        fs::write(self.output(), &bytes).map_err(|e| e.to_string())
    }
    /// Apply only after explicit user confirmation. Process exit alone never
    /// marks the file resolved. Failure keeps this object/output available.
    pub fn apply(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.path != self.root {
            return Err("The repository has changed".into());
        }
        let guard = repo.operation_guard(token)?;
        self.apply_reserved(repo, &guard, token)
    }

    pub(crate) fn apply_reserved(
        &self,
        repo: &Repository,
        guard: &crate::operation_queue::Guard,
        token: &Cancel,
    ) -> Result<(), String> {
        if repo.path != self.root || !guard.owns(&repo.operation_key(token)?) {
            return Err("Merge result does not belong to this update interaction".into());
        }
        self.snapshot.revalidate(repo, &self.file, token)?;
        let meta = fs::symlink_metadata(&self.inputs[3]).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("Merge output must be a regular file".into());
        }
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        let mut applied = self
            .applied
            .lock()
            .map_err(|_| "Merge result state is unavailable")?;
        // Freeze the exact bytes to be written before changing the worktree.
        // A later edit of result must not also change our expected disk baseline.
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let next = self.temporary.0.join(format!("applied-{random:x?}"));
        let result = (|| {
            let mut snapshot = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&next)
                .map_err(|e| e.to_string())?;
            crate::file_save::copy_stream(self.output(), &mut snapshot, token)
                .map_err(|e| format!("{e:?}"))?;
            snapshot.sync_all().map_err(|e| e.to_string())?;
            drop(snapshot);
            crate::file_save::save_from_file(
                &project,
                &self.file.path,
                applied.as_deref().unwrap_or(&self.expected),
                &next,
                token,
            )
            .map_err(|e| format!("{e:?}"))
        })();
        if result.is_ok() {
            // No fallible I/O between successful replacement and advancing the
            // baseline. Staging/cancellation can fail after this point; retry
            // must recognize our own write while still rejecting external edits.
            if let Some(previous) = applied.replace(next) {
                let _ = fs::remove_file(previous);
            }
        } else {
            let _ = fs::remove_file(next);
        }
        result
    }

    /// Read the last saved result for review, not the mutable editor output.
    /// The original conflict versions and current disk bytes must still match.
    pub fn saved_text(&self, repo: &Repository, token: &Cancel) -> Result<String, String> {
        if repo.path != self.root {
            return Err("The repository has changed".into());
        }
        let guard = repo.operation_guard(token)?;
        self.saved_text_reserved(repo, &guard, token)
    }

    pub(crate) fn saved_text_reserved(
        &self,
        repo: &Repository,
        guard: &crate::operation_queue::Guard,
        token: &Cancel,
    ) -> Result<String, String> {
        if repo.path != self.root || !guard.owns(&repo.operation_key(token)?) {
            return Err("Merge result does not belong to this update interaction".into());
        }
        self.snapshot.revalidate(repo, &self.file, token)?;
        let applied = self
            .applied
            .lock()
            .map_err(|_| "Merge result state is unavailable")?;
        let expected = applied
            .as_ref()
            .ok_or("Save the merge result before reviewing it")?;
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        let actual = project
            .resolve(&self.file.path, token)
            .map_err(|e| format!("{e:?}"))?;
        if !crate::file_save::same_file_bytes(expected, &actual, token)
            .map_err(|e| format!("{e:?}"))?
        {
            return Err(
                "Merge result changed on disk; review it before marking it resolved".into(),
            );
        }
        let mut bytes = Vec::new();
        fs::File::open(expected)
            .map_err(|e| e.to_string())?
            .take(crate::project_files::FILE_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let document = crate::file_document::Document::new(bytes)
            .map_err(|e| format!("Unsupported merge text: {e:?}"))?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(document.text().to_owned())
    }

    /// Finalize an explicitly saved result without rewriting the worktree.
    /// Rebased's iterative flow calls this when closing/finishing its conflict
    /// session. The immediate editor path uses the same check after apply.
    pub fn stage(
        &self,
        repo: &Repository,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<conflicts::ResolutionResult, String> {
        if repo.path != self.root {
            return Err("The repository has changed".into());
        }
        let guard = repo.operation_guard(token)?;
        self.stage_reserved(repo, &guard, token, progress)
    }

    pub(crate) fn stage_reserved(
        &self,
        repo: &Repository,
        guard: &crate::operation_queue::Guard,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<conflicts::ResolutionResult, String> {
        if repo.path != self.root {
            return Err("The repository has changed".into());
        }
        conflicts::resolve_reserved(
            repo,
            guard,
            &self.snapshot,
            &self.file,
            conflicts::Resolution::MarkResolved,
            token,
            progress,
            || {
                // Repository ownership is acquired before this mutex, just as
                // in apply; the two paths cannot deadlock or interleave saves.
                let applied = self
                    .applied
                    .lock()
                    .map_err(|_| "Merge result state is unavailable")?;
                let expected = applied
                    .as_ref()
                    .ok_or("Save the merge result before marking it resolved")?;
                let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
                let actual = project
                    .resolve(&self.file.path, token)
                    .map_err(|e| format!("{e:?}"))?;
                if !crate::file_save::same_file_bytes(expected, &actual, token)
                    .map_err(|e| format!("{e:?}"))?
                {
                    return Err(
                        "Merge result changed on disk; review it before marking it resolved".into(),
                    );
                }
                Ok(())
            },
        )
    }

    pub fn file(&self) -> &File {
        &self.file
    }
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    /// A failed write-back must not destroy the user's merge work.
    /// Retained files survive dialog/window closure and are explicitly reported
    /// to the user. Normal success/cancellation before apply still cleans up.
    pub fn retain_output(&self) -> &std::path::Path {
        self.temporary.1.store(true, Ordering::Relaxed);
        self.output()
    }
    pub fn output(&self) -> &std::path::Path {
        &self.inputs[3]
    }
    pub fn temporary_directory(&self) -> &std::path::Path {
        &self.temporary.0
    }
}
