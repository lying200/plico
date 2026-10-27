//! Small per-user layout preference, independent of repository data.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub const DEFAULT_FILES_FRACTION: f32 = 0.7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogFilesOptions {
    pub directory: bool,
    pub details: bool,
}
impl Default for LogFilesOptions {
    fn default() -> Self {
        Self {
            directory: true,
            details: true,
        }
    }
}
fn read_log_files_options(path: &Path) -> LogFilesOptions {
    let mut text = String::new();
    let mut result = LogFilesOptions::default();
    if std::fs::File::open(path)
        .and_then(|f| f.take(65).read_to_string(&mut text))
        .is_err()
        || text.len() > 64
    {
        return result;
    }
    for line in text.lines() {
        match line {
            "directory=true" => result.directory = true,
            "directory=false" => result.directory = false,
            "details=true" => result.details = true,
            "details=false" => result.details = false,
            _ => {}
        }
    }
    result
}
pub fn load_log_files_options() -> LogFilesOptions {
    settings_path()
        .map(|p| read_log_files_options(&p.with_file_name("log-files-options")))
        .unwrap_or_default()
}
pub fn save_log_files_options(options: LogFilesOptions) -> std::io::Result<()> {
    let Some(path) = settings_path().map(|p| p.with_file_name("log-files-options")) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(
        path,
        format!(
            "directory={}\ndetails={}\n",
            options.directory, options.details
        ),
    )
}

fn comparison_swapped_file(root: &Path) -> Option<(PathBuf, String)> {
    root.to_str()?;
    let key = crate::recent_projects::identity(root);
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    Some((
        settings_path()?.with_file_name(format!("comparison-swapped-{hash:016x}")),
        key,
    ))
}
pub fn load_comparison_swapped(root: &Path) -> bool {
    comparison_swapped_file(root)
        .and_then(|(path, key)| read_history_details(&path, &key))
        .unwrap_or(false)
}
pub fn save_comparison_swapped(root: &Path, swapped: bool) -> std::io::Result<()> {
    let Some((path, key)) = comparison_swapped_file(root) else {
        return Ok(());
    };
    save_project_boolean(&path, &key, swapped)
}

fn rebase_grouping_file(root: &Path) -> Option<(PathBuf, String)> {
    let (path, key) = history_details_file(root)?;
    let name = path.file_name()?.to_str()?;
    Some((path.with_file_name(format!("rebase-directory-{name}")), key))
}

pub fn load_rebase_grouping(root: &Path) -> bool {
    rebase_grouping_file(root)
        .and_then(|(path, key)| read_history_details(&path, &key))
        .unwrap_or(true)
}

pub fn save_rebase_grouping(root: &Path, grouped: bool) -> std::io::Result<()> {
    let Some((path, key)) = rebase_grouping_file(root) else {
        return Err(std::io::Error::other("Settings directory unavailable"));
    };
    save_project_boolean(&path, &key, grouped)
}

fn branch_preview_file(root: &Path, bottom: bool) -> Option<(PathBuf, String)> {
    let (path, key) = comparison_swapped_file(root)?;
    let name = path.file_name()?.to_str()?;
    Some((
        path.with_file_name(format!(
            "branch-preview-{}-{name}",
            if bottom { "bottom" } else { "top" }
        )),
        key,
    ))
}

pub fn load_branch_preview(root: &Path, bottom: bool) -> bool {
    branch_preview_file(root, bottom)
        .and_then(|(path, key)| read_history_details(&path, &key))
        .unwrap_or(false)
}

pub fn save_branch_preview(root: &Path, bottom: bool, shown: bool) -> std::io::Result<()> {
    let Some((path, key)) = branch_preview_file(root, bottom) else {
        return Ok(());
    };
    save_project_boolean(&path, &key, shown)
}

fn settings_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    base.map(|base| base.join("GitReview").join("files-details-ratio"))
}

fn valid(value: f32) -> bool {
    value.is_finite() && (0.1..=0.9).contains(&value)
}

fn read(path: &Path) -> Option<f32> {
    let mut text = String::new();
    std::fs::File::open(path)
        .ok()?
        .take(65)
        .read_to_string(&mut text)
        .ok()?;
    if text.len() > 64 {
        return None;
    }
    let value = text.trim().parse().ok()?;
    valid(value).then_some(value)
}

pub fn load_files_fraction() -> f32 {
    settings_path()
        .and_then(|path| read(&path))
        .unwrap_or(DEFAULT_FILES_FRACTION)
}

pub fn save_files_fraction(value: f32) -> std::io::Result<()> {
    if !valid(value) {
        return Ok(());
    }
    let Some(path) = settings_path() else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    // Persist only on drag completion; a partial write on abrupt termination
    // safely falls back to the default on the next startup.
    std::fs::write(path, format!("{value:.6}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebase_grouping_roundtrips_without_crossing_project_identity() {
        let root =
            std::env::temp_dir().join(format!("plico-rebase-grouping-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let (first, key) = rebase_grouping_file(&root.join("first")).unwrap();
        let (second, other_key) = rebase_grouping_file(&root.join("second")).unwrap();
        assert_ne!(first, second);
        let isolated = root.join(first.file_name().unwrap());
        for grouped in [false, true] {
            save_project_boolean(&isolated, &key, grouped).unwrap();
            assert_eq!(read_history_details(&isolated, &key), Some(grouped));
            assert_eq!(read_history_details(&isolated, &other_key), None);
        }
        assert!(save_project_boolean(&isolated, &other_key, false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn branch_preview_is_scoped_to_project_and_side() {
        let root =
            std::env::temp_dir().join(format!("plico-branch-preview-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let (top, key) = branch_preview_file(&root.join("repo-a"), false).unwrap();
        let (bottom, _) = branch_preview_file(&root.join("repo-a"), true).unwrap();
        let (other, other_key) = branch_preview_file(&root.join("repo-b"), false).unwrap();
        assert_ne!(top, bottom);
        assert_ne!(top, other);
        // Exercise the real encoding in an isolated directory, never user settings.
        let top = root.join(top.file_name().unwrap());
        let bottom = root.join(bottom.file_name().unwrap());
        save_project_boolean(&top, &key, true).unwrap();
        save_project_boolean(&bottom, &key, false).unwrap();
        assert_eq!(read_history_details(&top, &key), Some(true));
        assert_eq!(read_history_details(&bottom, &key), Some(false));
        assert_eq!(read_history_details(&top, &other_key), None);
        assert!(save_project_boolean(&top, &other_key, false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn log_files_options_are_independent_and_bounded() {
        let path =
            std::env::temp_dir().join(format!("plico-log-files-options-{}", std::process::id()));
        for directory in [false, true] {
            for details in [false, true] {
                std::fs::write(&path, format!("directory={directory}\ndetails={details}\n"))
                    .unwrap();
                assert_eq!(
                    read_log_files_options(&path),
                    LogFilesOptions { directory, details }
                );
            }
        }
        for text in ["directory=invalid".to_owned(), "x".repeat(65)] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(read_log_files_options(&path), LogFilesOptions::default());
        }
        std::fs::remove_file(&path).unwrap();
        assert_eq!(read_log_files_options(&path), LogFilesOptions::default());
    }
    #[test]
    fn rejects_invalid_and_oversized_preferences() {
        let path = std::env::temp_dir().join(format!("gitui-layout-test-{}", std::process::id()));
        for input in ["NaN", "inf", "-0.3", "1", "garbage", "", &" ".repeat(65)] {
            std::fs::write(&path, input).unwrap();
            assert_eq!(read(&path), None, "{input:?}");
        }
        std::fs::write(&path, "0.425000\n").unwrap();
        assert_eq!(read(&path), Some(0.425));
        std::fs::remove_file(path).unwrap();
    }
}

/// Missing or invalid preference keeps the default editor-tab layout.
pub fn load_bottom_preview() -> bool {
    let Some(path) = settings_path().map(|p| p.with_file_name("diff-location")) else {
        return false;
    };
    let mut value = String::new();
    std::fs::File::open(path)
        .ok()
        .and_then(|file| file.take(16).read_to_string(&mut value).ok());
    value.trim() == "bottom"
}
pub fn save_bottom_preview(bottom: bool) -> std::io::Result<()> {
    let Some(path) = settings_path().map(|p| p.with_file_name("diff-location")) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, if bottom { "bottom\n" } else { "tab\n" })
}

/// Rebased keeps viewer preference per Diff place, shared across repositories.
#[derive(Clone, Copy)]
pub enum DiffPlace {
    Log,
    Changes,
}
impl DiffPlace {
    fn file_name(self) -> &'static str {
        match self {
            Self::Log => "diff-view-log",
            Self::Changes => "diff-view-changes",
        }
    }
    fn default_unified(self) -> bool {
        matches!(self, Self::Changes)
    }
}
fn read_diff_unified(path: &Path, place: DiffPlace) -> bool {
    let mut value = String::new();
    let read = std::fs::File::open(path).and_then(|file| file.take(17).read_to_string(&mut value));
    if read.is_ok() && value.len() <= 16 {
        match value.trim() {
            "unified" => return true,
            "split" => return false,
            _ => {}
        }
    }
    place.default_unified()
}
pub fn load_diff_unified(place: DiffPlace) -> bool {
    settings_path()
        .map(|p| read_diff_unified(&p.with_file_name(place.file_name()), place))
        .unwrap_or(place.default_unified())
}
pub fn save_diff_unified(place: DiffPlace, unified: bool) -> std::io::Result<()> {
    let Some(path) = settings_path().map(|p| p.with_file_name(place.file_name())) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, if unified { "unified\n" } else { "split\n" })
}
#[cfg(test)]
mod diff_preference_tests {
    use super::*;
    #[test]
    fn viewer_places_have_distinct_defaults_and_reject_corrupt_values() {
        let path = std::env::temp_dir().join(format!("gitui-diff-mode-{}", std::process::id()));
        assert_ne!(DiffPlace::Log.file_name(), DiffPlace::Changes.file_name());
        for input in [
            "",
            "invalid",
            "unifiedxxxxxxxxxx",
            "splitxxxxxxxxxxxx",
            " unified         ",
        ] {
            std::fs::write(&path, input).unwrap();
            assert!(!read_diff_unified(&path, DiffPlace::Log));
            assert!(read_diff_unified(&path, DiffPlace::Changes));
        }
        for (input, expected) in [("unified\n", true), ("split\n", false)] {
            std::fs::write(&path, input).unwrap();
            assert_eq!(read_diff_unified(&path, DiffPlace::Log), expected);
            assert_eq!(read_diff_unified(&path, DiffPlace::Changes), expected);
        }
        std::fs::remove_file(&path).unwrap();
        assert!(!read_diff_unified(&path, DiffPlace::Log));
        assert!(read_diff_unified(&path, DiffPlace::Changes));
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiffEmphasis {
    Lines,
    #[default]
    Words,
    Characters,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiffAppearance {
    pub line_numbers: bool,
    pub soft_wrap: bool,
    pub emphasis: DiffEmphasis,
}
impl Default for DiffAppearance {
    fn default() -> Self {
        Self {
            line_numbers: true,
            soft_wrap: false,
            emphasis: DiffEmphasis::Words,
        }
    }
}
impl DiffAppearance {
    fn encode(self) -> String {
        format!(
            "line-numbers={}\nsoft-wrap={}\nemphasis={}\n",
            self.line_numbers,
            self.soft_wrap,
            match self.emphasis {
                DiffEmphasis::Lines => "lines",
                DiffEmphasis::Words => "words",
                DiffEmphasis::Characters => "characters",
            }
        )
    }
}
fn read_diff_appearance(path: &Path) -> DiffAppearance {
    let mut value = String::new();
    if std::fs::File::open(path)
        .and_then(|file| file.take(129).read_to_string(&mut value))
        .is_err()
        || value.len() > 128
    {
        return DiffAppearance::default();
    }
    let mut result = DiffAppearance::default();
    for line in value.lines() {
        match line {
            "line-numbers=true" => result.line_numbers = true,
            "line-numbers=false" => result.line_numbers = false,
            "soft-wrap=true" => result.soft_wrap = true,
            "soft-wrap=false" => result.soft_wrap = false,
            "emphasis=lines" => result.emphasis = DiffEmphasis::Lines,
            "emphasis=words" => result.emphasis = DiffEmphasis::Words,
            "emphasis=characters" => result.emphasis = DiffEmphasis::Characters,
            _ => {}
        }
    }
    result
}
fn appearance_path(place: DiffPlace) -> Option<PathBuf> {
    settings_path().map(|path| {
        path.with_file_name(match place {
            DiffPlace::Log => "diff-editor-log",
            DiffPlace::Changes => "diff-editor-changes",
        })
    })
}
pub fn load_diff_appearance(place: DiffPlace) -> DiffAppearance {
    appearance_path(place)
        .map(|path| read_diff_appearance(&path))
        .unwrap_or_default()
}
pub fn save_diff_appearance(place: DiffPlace, value: DiffAppearance) -> std::io::Result<()> {
    let Some(path) = appearance_path(place) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, value.encode())
}
#[cfg(test)]
mod appearance_tests {
    use super::*;
    #[test]
    fn appearance_roundtrip_and_invalid_fallback() {
        let path = std::env::temp_dir().join(format!("gitui-appearance-{}", std::process::id()));
        assert_eq!(read_diff_appearance(&path), DiffAppearance::default());
        for emphasis in [
            DiffEmphasis::Lines,
            DiffEmphasis::Words,
            DiffEmphasis::Characters,
        ] {
            for line_numbers in [true, false] {
                for soft_wrap in [true, false] {
                    let expected = DiffAppearance {
                        line_numbers,
                        soft_wrap,
                        emphasis,
                    };
                    std::fs::write(&path, expected.encode()).unwrap();
                    assert_eq!(read_diff_appearance(&path), expected);
                }
            }
        }
        for invalid in [
            "line-numbers=bad".to_owned(),
            "emphasis=unknown".into(),
            "x".repeat(129),
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert_eq!(read_diff_appearance(&path), DiffAppearance::default());
        }
        std::fs::remove_file(path).unwrap();
    }
}

pub fn load_language() -> crate::i18n::Language {
    let Some(path) = settings_path().map(|p| p.with_file_name("language")) else {
        return Default::default();
    };
    let mut value = String::new();
    if std::fs::File::open(path)
        .and_then(|f| f.take(33).read_to_string(&mut value))
        .is_err()
        || value.len() > 32
    {
        return Default::default();
    }
    crate::i18n::Language::parse(&value)
}
pub fn save_language(value: crate::i18n::Language) -> std::io::Result<()> {
    let path = settings_path()
        .map(|p| p.with_file_name("language"))
        .ok_or_else(|| std::io::Error::other("User configuration directory is unavailable"))?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, value.encode())
}

/// Project file previews are opt-in, matching the upstream editor default.
pub fn load_file_preview() -> bool {
    let Some(path) = settings_path().map(|p| p.with_file_name("file-preview")) else {
        return false;
    };
    let mut value = String::new();
    std::fs::File::open(path)
        .ok()
        .and_then(|file| file.take(16).read_to_string(&mut value).ok());
    value.trim() == "true"
}
pub fn save_file_preview(enabled: bool) -> std::io::Result<()> {
    let Some(path) = settings_path().map(|p| p.with_file_name("file-preview")) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, if enabled { "true\n" } else { "false\n" })
}

// Per-project file history properties live in app data, never in the worktree.
fn history_project_key(root: &Path) -> Option<String> {
    let text = root.to_str()?.replace('\\', "/");
    #[cfg(windows)]
    let text = text.to_lowercase();
    Some(text)
}
fn history_details_file(root: &Path) -> Option<(PathBuf, String)> {
    let key = history_project_key(root)?;
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    Some((
        settings_path()?.with_file_name(format!("file-history-details-{hash:016x}")),
        key,
    ))
}
fn read_history_details(file: &Path, key: &str) -> Option<bool> {
    let mut text = String::new();
    std::fs::File::open(file)
        .ok()?
        .take(65537)
        .read_to_string(&mut text)
        .ok()?;
    if text.len() > 65536 {
        return None;
    }
    let (stored, value) = text.split_once('\0')?;
    if stored != key {
        return None;
    }
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}
pub fn load_history_details(root: &Path) -> bool {
    history_details_file(root)
        .and_then(|(path, key)| read_history_details(&path, &key))
        .unwrap_or(false)
}
pub fn save_history_details(root: &Path, value: bool) -> std::io::Result<()> {
    let Some((path, key)) = history_details_file(root) else {
        return Ok(());
    };
    save_project_boolean(&path, &key, value)
}
fn save_project_boolean(path: &Path, key: &str, value: bool) -> std::io::Result<()> {
    if key.len() > 65530 {
        return Err(std::io::Error::other(
            "Project path exceeds settings budget",
        ));
    }
    // The stored identity checks the filename hash; a collision must not apply
    // another project's setting or silently overwrite it.
    if path.exists() {
        let mut stored = String::new();
        std::fs::File::open(&path)?
            .take(65537)
            .read_to_string(&mut stored)?;
        if stored.len() <= 65536 && stored.split_once('\0').is_some_and(|(k, _)| k != key) {
            return Err(std::io::Error::other("Invalid project settings identity"));
        }
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(
        path,
        format!("{key}\0{}", if value { "true" } else { "false" }),
    )
}

fn log_long_edges_file(root: &Path) -> Option<(PathBuf, String)> {
    root.to_str()?;
    let key = crate::recent_projects::identity(root);
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    Some((
        settings_path()?.with_file_name(format!("log-long-edges-{hash:016x}")),
        key,
    ))
}
pub fn load_log_long_edges(root: &Path) -> bool {
    log_long_edges_file(root)
        .and_then(|(path, key)| read_history_details(&path, &key))
        .unwrap_or(false)
}
pub fn save_log_long_edges(root: &Path, value: bool) -> std::io::Result<()> {
    let Some((path, key)) = log_long_edges_file(root) else {
        return Ok(());
    };
    save_project_boolean(&path, &key, value)
}

#[cfg(test)]
mod log_graph_preference_tests {
    use super::*;
    #[test]
    fn project_boolean_round_trip_rejects_other_project_and_bad_records() {
        let path = std::env::temp_dir().join(format!("gitui-long-edges-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_history_details(&path, "one"), None);
        for value in [true, false] {
            save_project_boolean(&path, "one", value).unwrap();
            assert_eq!(read_history_details(&path, "one"), Some(value));
            assert_eq!(read_history_details(&path, "two"), None);
        }
        let before = std::fs::read(&path).unwrap();
        assert!(save_project_boolean(&path, "two", true).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::write(&path, "one\0invalid").unwrap();
        assert_eq!(read_history_details(&path, "one"), None);
        std::fs::write(&path, "x".repeat(65537)).unwrap();
        assert_eq!(read_history_details(&path, "one"), None);
        std::fs::remove_file(path).unwrap();
    }
}

// Rebased PushSettings: remembered confirmations belong to a project and exact
// remote/branch pair. Hash only selects the file; stored identity is rechecked.
fn force_push_confirmation_file(
    root: &Path,
    remote: &str,
    branch: &str,
) -> Option<(PathBuf, String)> {
    root.to_str()?;
    let project = crate::recent_projects::identity(root);
    let key = format!(
        "{}:{}{}:{}{}:{}",
        project.len(),
        project,
        remote.len(),
        remote,
        branch.len(),
        branch
    );
    if key.len() > 65530 || key.contains('\0') {
        return None;
    }
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    Some((
        settings_path()?.with_file_name(format!("force-push-confirmation-{hash:016x}")),
        key,
    ))
}
pub fn silent_force_push(root: &Path, remote: &str, branch: &str) -> bool {
    force_push_confirmation_file(root, remote, branch)
        .and_then(|(file, key)| read_history_details(&file, &key))
        .unwrap_or(false)
}
pub fn remember_force_push(root: &Path, remote: &str, branch: &str) -> std::io::Result<()> {
    let (file, key) = force_push_confirmation_file(root, remote, branch)
        .ok_or_else(|| std::io::Error::other("Force-push preference identity is unavailable"))?;
    save_project_boolean(&file, &key, true)
}

#[cfg(test)]
mod force_confirmation_tests {
    use super::*;
    #[test]
    fn remembered_force_target_checks_project_remote_and_branch_identity() {
        let root = std::env::temp_dir();
        let (_, key) = force_push_confirmation_file(&root, "origin", "topic").unwrap();
        let (_, other) = force_push_confirmation_file(&root, "origin", "other").unwrap();
        let (_, project) =
            force_push_confirmation_file(&root.join("other"), "origin", "topic").unwrap();
        let (_, remote) = force_push_confirmation_file(&root, "upstream", "topic").unwrap();
        let file = root.join(format!(
            "plico-force-preference-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&file);
        save_project_boolean(&file, &key, true).unwrap();
        assert_eq!(read_history_details(&file, &key), Some(true));
        for other in [&other, &project, &remote] {
            assert_eq!(read_history_details(&file, other), None);
            assert!(save_project_boolean(&file, other, true).is_err());
        }
        std::fs::write(&file, "truncated").unwrap();
        assert_eq!(read_history_details(&file, &key), None);
        std::fs::remove_file(file).unwrap();
    }
}

pub fn load_reset_mode() -> crate::reset_operation::Mode {
    let value = settings_path()
        .and_then(|p| std::fs::File::open(p.with_file_name("reset-mode")).ok())
        .and_then(|f| {
            let mut s = String::new();
            f.take(16).read_to_string(&mut s).ok().map(|_| s)
        })
        .unwrap_or_default();
    crate::reset_operation::Mode::ALL
        .into_iter()
        .find(|m| m.name() == value.trim())
        .unwrap_or_default()
}
pub fn save_reset_mode(mode: crate::reset_operation::Mode) -> std::io::Result<()> {
    let path = settings_path()
        .ok_or_else(|| std::io::Error::other("Settings directory unavailable"))?
        .with_file_name("reset-mode");
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, mode.name())
}

#[derive(Clone, Copy)]
pub enum ChangesOption {
    Directory,
    Ignored,
    DeleteAdded,
}
fn changes_option_file(root: &Path, option: ChangesOption) -> Option<(PathBuf, String)> {
    let (path, key) = history_details_file(root)?;
    let name = match option {
        ChangesOption::Directory => "changes-directory",
        ChangesOption::Ignored => "changes-ignored",
        ChangesOption::DeleteAdded => "rollback-delete-added",
    };
    let filename = path.file_name()?.to_str()?;
    Some((path.with_file_name(format!("{name}-{filename}")), key))
}
pub fn load_changes_option(root: &Path, option: ChangesOption) -> bool {
    changes_option_file(root, option)
        .and_then(|(path, key)| read_history_details(&path, &key))
        .unwrap_or(false)
}
pub fn save_changes_option(root: &Path, option: ChangesOption, value: bool) -> std::io::Result<()> {
    let Some((path, key)) = changes_option_file(root, option) else {
        return Ok(());
    };
    save_project_boolean(&path, &key, value)
}

pub fn load_preview_fraction() -> f32 {
    settings_path()
        .and_then(|p| read(&p.with_file_name("log-preview-ratio")))
        .unwrap_or(0.7)
}
pub fn save_preview_fraction(value: f32) -> std::io::Result<()> {
    if !valid(value) {
        return Ok(());
    }
    let Some(path) = settings_path().map(|p| p.with_file_name("log-preview-ratio")) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, format!("{value:.6}\n"))
}

pub fn load_tool_window_order() -> crate::tool_window_order::Order {
    let mut text = String::new();
    let loaded = settings_path()
        .and_then(|p| std::fs::File::open(p.with_file_name("tool-window-order")).ok())
        .and_then(|f| f.take(257).read_to_string(&mut text).ok());
    if loaded.is_none() || text.len() > 256 {
        return Default::default();
    }
    crate::tool_window_order::Order::parse(&text)
}
pub fn save_tool_window_order(order: &crate::tool_window_order::Order) -> std::io::Result<()> {
    let Some(path) = settings_path().map(|p| p.with_file_name("tool-window-order")) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, order.encode())
}

/// Follow editor selection in Project; opt-in, like ProjectViewState.
pub fn load_project_auto_follow() -> bool {
    let mut value = String::new();
    settings_path()
        .and_then(|p| std::fs::File::open(p.with_file_name("project-auto-follow")).ok())
        .and_then(|f| f.take(16).read_to_string(&mut value).ok());
    value.trim() == "true"
}
pub fn save_project_auto_follow(enabled: bool) -> std::io::Result<()> {
    let path = settings_path()
        .ok_or_else(|| std::io::Error::other("Settings directory unavailable"))?
        .with_file_name("project-auto-follow");
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, if enabled { "true" } else { "false" })
}

/// Markdown layout is a global default for newly opened documents, as in TextEditorWithPreview.
pub fn load_markdown_layout() -> String {
    settings_path()
        .and_then(|p| std::fs::File::open(p.with_file_name("markdown-layout")).ok())
        .and_then(|f| {
            let mut s = String::new();
            f.take(32).read_to_string(&mut s).ok().map(|_| s)
        })
        .unwrap_or_default()
}
pub fn save_markdown_layout(layout: &str) -> std::io::Result<()> {
    if !matches!(layout, "source" | "split" | "preview") {
        return Ok(());
    }
    let Some(path) = settings_path().map(|p| p.with_file_name("markdown-layout")) else {
        return Ok(());
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, layout)
}

#[derive(Clone, Copy)]
pub enum RebaseSplit {
    Plan,
    Details,
}
impl RebaseSplit {
    fn file(self) -> &'static str {
        match self {
            Self::Plan => "rebase-plan-ratio",
            Self::Details => "rebase-details-ratio",
        }
    }
    pub fn load(self) -> f32 {
        settings_path()
            .and_then(|p| read(&p.with_file_name(self.file())))
            .unwrap_or(match self {
                Self::Plan => 0.5,
                Self::Details => 0.67,
            })
    }
    pub fn save(self, value: f32) -> std::io::Result<()> {
        if !valid(value) {
            return Ok(());
        }
        let Some(path) = settings_path().map(|p| p.with_file_name(self.file())) else {
            return Ok(());
        };
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, format!("{value:.6}\n"))
    }
}
