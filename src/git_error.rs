//! Presentation classification only; Git diagnostics remain intact for details.
pub fn is_ownership_error(error: &str) -> bool {
    error
        .lines()
        .any(|line| line.contains("fatal: detected dubious ownership in repository at "))
}

pub fn summary(error: &str) -> &'static str {
    if is_ownership_error(error) {
        "Git blocked this repository because its owner differs"
    } else {
        "Could not load repository data"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ownership_diagnostic_is_classified_without_extracting_or_executing_commands() {
        let error = "fatal: detected dubious ownership in repository at '//wsl.localhost/host/home/user/repo'\nTo add an exception for this directory, call:\n\ngit config --global --add safe.directory 'repo'";
        assert!(super::is_ownership_error(error));
        assert!(!super::is_ownership_error("fatal: not a git repository"));
        assert!(!super::is_ownership_error(
            "commit mentions dubious ownership"
        ));
    }
}
