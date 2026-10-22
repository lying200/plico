//! Presentation of Commit reports, independent of GPUI and Git execution.
//! Report::status owns success/error precedence; recovery diagnostics precede
//! process output so a noisy hook cannot hide the reason recovery is required.
use crate::commit_operation::{Report, Status};

const LIMIT: usize = 32_000;
const TRUNCATED: &str = "\n[Diagnostic truncated]";

#[derive(Debug)]
pub struct Failure {
    pub summary: &'static str,
    pub details: String,
}

impl Failure {
    pub fn commit(details: String) -> Self {
        Self::new("Commit failed", &details)
    }
    pub fn restore(details: String) -> Self {
        Self::new("Could not restore staged changes", &details)
    }
    fn new(summary: &'static str, details: &str) -> Self {
        let mut text = Diagnostic::default();
        text.push(details);
        Self {
            summary,
            details: text.finish(),
        }
    }
}

/// No retries, repository writes, or draft changes. Success remains authoritative
/// even if a successful command printed error-looking text.
pub fn from_report(report: &Report) -> Result<(), Failure> {
    let summary = match report.status() {
        Status::Success => return Ok(()),
        Status::RestoreFailed => "Commit index recovery is required",
        Status::RecoveryRecordFailed => "Could not remove commit recovery record",
        Status::Cancelled => "Commit cancelled",
        Status::TimedOut => "Commit timed out",
        Status::SigningFailed => "Commit signing failed",
        Status::Empty => "Nothing to commit",
        Status::Failed => "Commit failed",
    };
    let mut text = Diagnostic::default();
    for error in [
        report.restore.as_ref().err(),
        report.recovery_record_error.as_ref(),
        report.head.as_ref().err(),
        report.state.as_ref().err(),
    ]
    .into_iter()
    .flatten()
    {
        text.push(error);
    }
    if let Some(step) = report.steps.last() {
        match &step.result {
            Err(error) => text.push(error),
            Ok(out) => {
                if let Some(error) = &out.input_error {
                    text.push(&error.to_string());
                }
                text.push(&String::from_utf8_lossy(&out.stderr.bytes()));
                text.push(&String::from_utf8_lossy(&out.stdout.bytes()));
                text.truncated |= out.stderr.truncated || out.stdout.truncated;
            }
        }
    }
    Err(Failure {
        summary,
        details: text.finish(),
    })
}

#[derive(Default)]
struct Diagnostic {
    text: String,
    chars: usize,
    truncated: bool,
}
impl Diagnostic {
    fn push(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        // Include the marker in the bound, and never split a UTF-8 character.
        let budget = LIMIT - TRUNCATED.chars().count();
        if self.chars >= budget {
            self.truncated = true;
            return;
        }
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
            self.chars += 1;
        }
        let mut chars = value.chars();
        for ch in chars.by_ref().take(budget - self.chars) {
            self.text.push(ch);
            self.chars += 1;
        }
        self.truncated |= chars.next().is_some();
    }
    fn finish(mut self) -> String {
        if self.truncated {
            self.text.push_str(TRUNCATED);
        }
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        commit_operation::{Phase, Step},
        operation_state::{Operation, State},
    };

    fn report() -> Report {
        Report {
            steps: vec![Step {
                phase: Phase::Commit,
                result: Err("hook failed".into()),
            }],
            cancelled: false,
            restore: Ok(()),
            recovery_input: None,
            recovery_record: None,
            recovery_record_error: None,
            head: Ok(None),
            state: Ok(State {
                operation: Operation::None,
                detached: false,
                rebase_branch: None,
                conflicts: vec![],
            }),
        }
    }
    #[test]
    fn recovery_survives_noisy_unicode_output_and_takes_priority_over_cancel() {
        let mut report = report();
        report.steps[0].result = Err("日志".repeat(LIMIT));
        report.cancelled = true;
        report.restore = Err("restore index failed".into());
        report.recovery_record_error = Some("recovery record retained".into());
        report.head = Err("cannot read HEAD".into());
        report.state = Err("cannot read state".into());
        let error = from_report(&report).unwrap_err();
        assert_eq!(error.summary, "Commit index recovery is required");
        assert!(error.details.starts_with(
            "restore index failed\nrecovery record retained\ncannot read HEAD\ncannot read state\n"
        ));
        assert!(error.details.ends_with(TRUNCATED));
        assert!(error.details.chars().count() <= LIMIT);
    }
    #[test]
    fn record_cleanup_cancel_and_generic_errors_keep_their_meaning() {
        let mut report = report();
        assert_eq!(from_report(&report).unwrap_err().details, "hook failed");
        report.cancelled = true;
        assert_eq!(
            from_report(&report).unwrap_err().summary,
            "Commit cancelled"
        );
        report.recovery_record_error = Some("record locked".into());
        let error = from_report(&report).unwrap_err();
        assert_eq!(error.summary, "Could not remove commit recovery record");
        assert_eq!(error.details, "record locked\nhook failed");
    }
    #[test]
    fn preflight_and_recovery_errors_are_also_bounded() {
        assert_eq!(Failure::commit("stale HEAD".into()).details, "stale HEAD");
        let failure = Failure::restore("界".repeat(LIMIT * 2));
        assert_eq!(failure.summary, "Could not restore staged changes");
        assert_eq!(failure.details.chars().count(), LIMIT);
        assert!(failure.details.ends_with(TRUNCATED));
    }
}
