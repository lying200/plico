//! Gutter chunk commits, following Rebased CommitChunkWorkflow and
//! GitCheckinEnvironment.addPartialChangesToIndex (4aec352, Apache-2.0).
//! Compose a blob from HEAD plus explicit ranges; never rewrite the worktree.
use crate::{
    file_document::Document,
    git::{Cancel, Repository},
    line_changes::{self, Block},
    operation_process::{self, Progress},
};
use std::{ffi::OsStr, path::PathBuf, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct Chunk {
    pub path: PathBuf,
    pub base_oid: String,
    pub current: Arc<str>,
    pub block: Block,
    pub message: String,
}

pub fn execute(
    repo: &Repository,
    chunk: &Chunk,
    token: &Cancel,
    progress: &Progress,
) -> Result<crate::commit_operation::Report, String> {
    let request = crate::commit_operation::Request {
        add: vec![chunk.path.clone()],
        remove: vec![],
        message: chunk.message.clone(),
        amend: false,
        expected_head: crate::commit_operation::head(repo, token)?,
    };
    crate::commit_operation::execute_chunk(repo, &request, chunk, token, progress)
}

/// Called only inside the normal commit operation guard, before index mutation.
pub(crate) fn prepare(
    repo: &Repository,
    chunk: &Chunk,
    token: &Cancel,
    progress: &Progress,
) -> Result<Vec<u8>, String> {
    if line_changes::base_oid(repo, &chunk.path, token)?.as_deref() != Some(&chunk.base_oid) {
        return Err("Git baseline changed. Reopen the change marker.".into());
    }
    let original = line_changes::base_text(repo, &chunk.base_oid, token)?;
    let content = compose(&original, &chunk.current, &chunk.block)?;
    prepare_content(repo, &chunk.path, &chunk.current, content, token, progress)
}
pub(crate) fn prepare_file(
    repo: &Repository,
    file: &crate::partial_selection::File,
    token: &Cancel,
    progress: &Progress,
) -> Result<Vec<u8>, String> {
    let oid = line_changes::base_oid(repo, &file.path, token)?
        .ok_or("Partial commit requires a tracked regular file")?;
    if line_changes::base_text(repo, &oid, token)? != file.selection.original.as_ref() {
        return Err("Git baseline changed. Reopen the diff.".into());
    }
    if file.selection.counts().0 == 0 {
        return Err("Select changes to commit".into());
    }
    prepare_content(
        repo,
        &file.path,
        &file.selection.current,
        file.selection.compose()?,
        token,
        progress,
    )
}
fn prepare_content(
    repo: &Repository,
    path: &std::path::Path,
    current: &str,
    content: String,
    token: &Cancel,
    progress: &Progress,
) -> Result<Vec<u8>, String> {
    let project =
        crate::project_files::Project::new(repo.path.clone()).map_err(|e| format!("{e:?}"))?;
    let disk = project
        .read_bytes(path, token)
        .map_err(|e| format!("{e:?}"))?;
    let mut document = Document::new(disk.clone()).map_err(|e| format!("{e:?}"))?;
    if document.text() != current {
        return Err("File changed. Reopen the change marker before committing.".into());
    }
    document.edit(content).map_err(|e| format!("{e:?}"))?;
    let bytes = document
        .begin_save()
        .map_err(|e| format!("{e:?}"))?
        .map(|s| s.bytes)
        .unwrap_or_else(|| document.disk());
    // --path applies the same clean filters / autocrlf as GitIndexUtil.hashObject.
    let out = operation_process::run_with_input(
        repo.operation_command().args([
            OsStr::new("hash-object"),
            OsStr::new("-w"),
            OsStr::new("--stdin"),
            OsStr::new("--path"),
            path.as_os_str(),
            OsStr::new("--"),
        ]),
        token,
        progress,
        Some(Duration::from_secs(30)),
        &bytes,
    )
    .map_err(|e| e.to_string())?;
    if !out.status.success() || out.stopped.is_some() || out.input_error.is_some() {
        return Err(format!(
            "Could not prepare selected change: {}",
            String::from_utf8_lossy(&out.stderr.bytes())
        ));
    }
    let oid = String::from_utf8(out.stdout.bytes())
        .map_err(|e| e.to_string())?
        .trim()
        .to_owned();
    if !crate::git::is_oid(&oid) {
        return Err("Invalid partial commit blob".into());
    }
    // A filter can take time. Recheck the source after it completes.
    if project
        .read_bytes(path, token)
        .map_err(|e| format!("{e:?}"))?
        != disk
    {
        return Err("File changed while preparing the commit".into());
    }
    let staged = repo.read(
        &[
            OsStr::new("--literal-pathspecs"),
            OsStr::new("ls-files"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
            path.as_os_str(),
        ],
        token,
    )?;
    let mode = staged
        .split(|b| *b == b' ')
        .next()
        .ok_or("Missing index mode")?;
    if !matches!(mode, b"100644" | b"100755") {
        return Err("Partial commit requires a tracked regular file".into());
    }
    let path = crate::commit_operation::path_input(&[path.to_path_buf()])?;
    let mut input = mode.to_vec();
    input.push(b' ');
    input.extend_from_slice(oid.as_bytes());
    input.extend_from_slice(b" 0\t");
    input.extend_from_slice(&path);
    Ok(input)
}

pub fn compose(original: &str, current: &str, block: &Block) -> Result<String, String> {
    // Revalidate the exact range instead of silently committing a shifted hunk.
    if !line_changes::blocks(original, current).contains(block) {
        return Err("Change range is stale. Reopen the change marker.".into());
    }
    let mut text = line_changes::fragment(original, 0..block.original.start);
    text.push_str(&line_changes::fragment(current, block.current.clone()));
    text.extend(original.split_inclusive('\n').skip(block.original.end));
    Ok(text)
}
