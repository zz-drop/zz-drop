use crate::commands::{EXIT_OK, EXIT_PROVIDER_ERROR};
use crate::output;

#[derive(Debug, Clone, Copy, Default)]
pub struct BatchSummary {
    pub successes: u32,
    pub failures: u32,
    pub skipped: u32,
}

impl BatchSummary {
    pub fn record_success(&mut self) {
        self.successes += 1;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
    }

    pub fn record_skip(&mut self) {
        self.skipped += 1;
    }

    pub fn total_attempted(&self) -> u32 {
        self.successes + self.failures
    }

    pub fn exit_code(&self) -> i32 {
        if self.failures > 0 {
            EXIT_PROVIDER_ERROR
        } else {
            EXIT_OK
        }
    }

    /// Convenience for command runners: emit a `batch_summary`
    /// record (JSON mode only — text/quiet stay quiet on this) and
    /// return the exit code in one step. Skipped files are not
    /// counted in `total`/`ok`/`failed` because skips aren't
    /// attempts — they're a parser-level decision.
    pub fn emit_and_exit_code(&self) -> i32 {
        let exit = self.exit_code();
        output::emit_batch_summary(self.total_attempted(), self.successes, self.failures, exit);
        exit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_batch_is_ok() {
        let s = BatchSummary::default();
        assert_eq!(s.exit_code(), EXIT_OK);
    }

    #[test]
    fn all_success_is_ok() {
        let mut s = BatchSummary::default();
        s.record_success();
        s.record_success();
        assert_eq!(s.exit_code(), EXIT_OK);
        assert_eq!(s.total_attempted(), 2);
    }

    #[test]
    fn one_failure_flips_exit_code() {
        let mut s = BatchSummary::default();
        s.record_success();
        s.record_failure();
        assert_eq!(s.exit_code(), EXIT_PROVIDER_ERROR);
        assert_eq!(s.total_attempted(), 2);
    }

    #[test]
    fn skipped_does_not_count_as_failure() {
        let mut s = BatchSummary::default();
        s.record_skip();
        assert_eq!(s.exit_code(), EXIT_OK);
        assert_eq!(s.total_attempted(), 0);
    }
}
