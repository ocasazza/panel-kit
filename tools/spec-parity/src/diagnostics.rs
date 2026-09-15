//! Shared diagnostic report formatting for parity failures.

use std::fmt;

/// Human-readable ordered diagnostics emitted by a parity stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticReport {
    lines: Vec<String>,
}

impl DiagnosticReport {
    /// Create a report from already ordered diagnostic lines.
    pub(crate) fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }
}

impl fmt::Display for DiagnosticReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DiagnosticReport {}
