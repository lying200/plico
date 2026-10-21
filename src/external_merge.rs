//! ExternalDiffToolUtil's isolated input/output files and explicit resolution.
//! Reference Rebased 4aec352 (JetBrains and contributors, Apache-2.0).
use crate::{
    conflicts::{self, File, Snapshot},
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
    project_files::Project,
};
use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};

/// Arguments are separate argv values, not shell commands. Rebased patterns:
/// %1 yours, %2 theirs, %3 base, %4 output.
#[derive(Clone, Debug)]
pub struct Tool {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
}
/// Quoted argv parsing, with literal Windows backslashes. No variable or shell expansion.
pub fn parse_arguments(pattern: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut started = false;
    for c in pattern.chars() {
        if c == '\0' {
            return Err("Invalid argument character".into());
        }
        if quote == Some(c) {
            quote = None;
            started = true;
        } else if quote.is_none() && matches!(c, '\'' | '"') {
            quote = Some(c);
            started = true;
        } else if quote.is_none() && c.is_whitespace() {
            if started {
                args.push(std::mem::take(&mut current));
                started = false;
            }
        } else {
            current.push(c);
            started = true;
        }
    }
    if quote.is_some() {
        return Err("Unclosed quote in tool arguments".into());
    }
    if started {
        args.push(current);
    }
    Ok(args)
}
pub fn load_configuration() -> (String, String) {
    configuration_path()
        .and_then(|p| fs::File::open(p).ok())
        .and_then(|file| {
            let mut text = String::new();
            file.take(16385).read_to_string(&mut text).ok()?;
            if text.len() > 16384 {
                return None;
            }
            let (program, pattern) = text.split_once('\n')?;
            Some((program.to_owned(), pattern.to_owned()))
        })
        .unwrap_or_else(|| (String::new(), "%1 %2 %3 %4".into()))
}
pub fn save_configuration(program: &str, pattern: &str) -> Result<Tool, String> {
    if program.trim().is_empty()
        || program.contains(['\n', '\r', '\0'])
        || pattern.contains(['\n', '\r'])
    {
        return Err("Choose an executable and a single-line argument pattern".into());
    }
    let arguments = parse_arguments(pattern)?;
    if !arguments.iter().any(|a| a.contains("%4")) {
        return Err("Tool arguments must include %4 (output)".into());
    }
    if program.len() + pattern.len() + 1 > 16384 {
        return Err("Tool configuration is too long".into());
    }
    let path = configuration_path().ok_or("Application settings directory unavailable")?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(path, format!("{program}\n{pattern}")).map_err(|e| e.to_string())?;
    Ok(Tool {
        executable: program.into(),
        arguments,
    })
}
fn configuration_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    base.map(|p| p.join("GitReview/external-merge-tool"))
}

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
            return Err("Working file changed while preparing external merge".into());
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
            root: repo.path.clone(),
            inputs,
        })
    }
    pub fn run(&self, tool: &Tool, token: &Cancel, progress: &Progress) -> Result<Outcome, String> {
        if tool.executable.as_os_str().is_empty()
            || !tool.arguments.iter().any(|a| a.contains("%4"))
        {
            return Err("Choose a merge tool and include %4 (output) in its arguments".into());
        }
        let mut command = Command::new(&tool.executable);
        command.current_dir(&self.root);
        for argument in &tool.arguments {
            command.arg(expand(argument, &self.inputs));
        }
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string())
    }
    /// Apply only after explicit user confirmation. Process exit alone never
    /// marks the file resolved. Failure keeps this object/output available.
    pub fn apply(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.path != self.root {
            return Err("The repository has changed".into());
        }
        let _guard = repo.operation_guard(token)?;
        self.snapshot.revalidate(repo, &self.file, token)?;
        let meta = fs::symlink_metadata(&self.inputs[3]).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("Merge output must be a regular file".into());
        }
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        crate::file_save::save_from_file(
            &project,
            &self.file.path,
            &self.expected,
            &self.inputs[3],
            token,
        )
        .map_err(|e| format!("{e:?}"))
    }

    pub fn file(&self) -> &File {
        &self.file
    }
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    /// A failed write-back must not destroy the user's external tool work.
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
fn expand(argument: &str, inputs: &[PathBuf; 4]) -> OsString {
    let mut result = OsString::new();
    let mut rest = argument;
    while let Some(index) = rest.find('%') {
        result.push(&rest[..index]);
        rest = &rest[index..];
        match rest.as_bytes().get(1) {
            Some(value @ b'1'..=b'4') => {
                result.push(&inputs[(value - b'1') as usize]);
                rest = &rest[2..];
            }
            _ => {
                result.push("%");
                rest = &rest[1..];
            }
        }
    }
    result.push(rest);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argument_pattern_preserves_windows_paths_and_quoted_spaces() {
        assert_eq!(
            parse_arguments(r#"--merge "C:\Program Files\tool" '%1' %2 %3 %4"#).unwrap(),
            vec!["--merge", r"C:\Program Files\tool", "%1", "%2", "%3", "%4"]
        );
        assert!(parse_arguments("\"unterminated").is_err());
        assert!(parse_arguments("bad\0argument").is_err());
    }
    #[test]
    fn expands_without_shell_or_recursive_substitution() {
        let paths = ["ours %2", "theirs", "base", "output file"].map(PathBuf::from);
        assert_eq!(
            expand("--result=%4", &paths),
            OsString::from("--result=output file")
        );
        assert_eq!(
            expand("%1 %2 %unknown", &paths),
            OsString::from("ours %2 theirs %unknown")
        );
    }
}
