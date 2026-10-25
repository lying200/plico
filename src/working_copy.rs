//! Exact endpoints for an editable local diff. A patch is presentation data;
//! it must never become the disk baseline used to save a working file.
use crate::{
    diff_target::Target,
    file_document::Document,
    git::{Area, Cancel},
    project_files::Project,
};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

pub struct Contents {
    pub original: String,
    pub working: Document,
}

pub fn read(target: &Target, cancel: &Cancel) -> Result<Contents, String> {
    if let Some(change) = &target.identity().working_comparison {
        return change
            .read_editable(
                &crate::git::Repository::new(target.identity().repository.clone()),
                cancel,
            )
            .map(|c| c.contents);
    }
    let path = target
        .working_tree_document()
        .ok_or("Diff has no editable working-tree endpoint")?;
    let id = target.identity();
    let project = Project::new(&id.repository).map_err(|e| format!("{e:?}"))?;
    let bytes = project
        .read_bytes(path, cancel)
        .map_err(|e| format!("{e:?}"))?;
    let working = Document::new(bytes).map_err(|e| format!("{e:?}"))?;
    let original = read_base(target, cancel)?;
    Ok(Contents { original, working })
}

pub fn read_base(target: &Target, cancel: &Cancel) -> Result<String, String> {
    let path = &target.identity().path;
    let id = target.identity();
    let repo = crate::git::Repository::new(id.repository.clone());
    if let Some(change) = &id.working_comparison {
        return change.read_baseline(&repo, cancel);
    }
    let original = {
        let old = id.old_path.as_deref().unwrap_or(path);
        validate_path(old)?;
        // Resolve the object first, then read by immutable ID. In particular an
        // index refresh between these commands cannot mix two index versions.
        let listing = if id.area == Area::Unstaged {
            repo.read(
                &[
                    OsStr::new("--literal-pathspecs"),
                    OsStr::new("ls-files"),
                    OsStr::new("--stage"),
                    OsStr::new("-z"),
                    OsStr::new("--"),
                    old.as_os_str(),
                ],
                cancel,
            )?
        } else {
            // A formerly untracked/added tab can outlive an external commit.
            // Resolve today's HEAD, rather than trusting the original status letter.
            let head = repo.read(
                &[
                    OsStr::new("rev-parse"),
                    OsStr::new("--revs-only"),
                    OsStr::new("HEAD"),
                ],
                cancel,
            )?;
            let head = std::str::from_utf8(&head)
                .map_err(|_| "Invalid HEAD")?
                .trim();
            if head.is_empty() {
                return Ok(String::new());
            }
            if !crate::git::is_oid(head) {
                return Err("Invalid HEAD".into());
            }
            let read = |path: &Path| {
                repo.read(
                    &[
                        OsStr::new("--literal-pathspecs"),
                        OsStr::new("ls-tree"),
                        OsStr::new("-z"),
                        OsStr::new(head),
                        OsStr::new("--"),
                        path.as_os_str(),
                    ],
                    cancel,
                )
            };
            let listing = read(old)?;
            if listing.is_empty() && old != path {
                validate_path(path)?;
                read(path)?
            } else {
                listing
            }
        };
        let records: Vec<_> = listing
            .split(|b| *b == 0)
            .filter(|r| !r.is_empty())
            .collect();
        if records.is_empty() {
            return Ok(String::new());
        }
        if records.len() != 1 {
            return Err("Diff base changed or is unmerged; refresh before editing".into());
        }
        let header_end = records[0]
            .iter()
            .position(|b| *b == b'\t')
            .ok_or("Invalid base object record")?;
        let header = &records[0][..header_end];
        let fields: Vec<_> = header.split(|b| *b == b' ').collect();
        if fields.len() != 3 || !matches!(fields[0], b"100644" | b"100755") {
            return Err("Diff base is not a regular file".into());
        }
        let oid = if id.area == Area::Unstaged {
            if fields[2] != b"0" {
                return Err("Unmerged index cannot be edited as a normal diff".into());
            }
            fields[1]
        } else {
            if fields[1] != b"blob" {
                return Err("Diff base is not a blob".into());
            }
            fields[2]
        };
        let oid = std::str::from_utf8(oid).map_err(|_| "Invalid base object ID")?;
        if !crate::git::is_oid(oid) {
            return Err("Invalid base object ID".into());
        }
        let oid = OsString::from(oid);
        let bytes = repo.read(&[OsStr::new("cat-file"), OsStr::new("blob"), &oid], cancel)?;
        let document = Document::new(bytes).map_err(|e| format!("Unsupported diff base: {e:?}"))?;
        document.comparison_text(document.text())
    };
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(original)
}

fn validate_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return Err("Invalid local diff path".into());
    }
    Ok(())
}
