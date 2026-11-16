// SPDX-License-Identifier: GPL-3.0-only
//! Line-based DDL comparison used to validate the MetaDictionary port against
//! the Windows sample (`ddl_CLIENTE_win.sql`). CRLF/CR are normalized and the
//! final newline is ignored; everything else must match exactly.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdlComparison {
    pub identical: bool,
    pub total_lines: usize,
    pub differing_lines: usize,
    pub first_difference: Option<usize>,
    pub expected_line: Option<String>,
    pub actual_line: Option<String>,
}

impl DdlComparison {
    pub fn report(&self) -> String {
        if self.identical {
            return format!("DDL identical: {} lines", self.total_lines);
        }
        format!(
            "DDL differs: {} of {} lines\nfirst difference at line {}\nexpected (Windows): {}\nactual (Rust): {}",
            self.differing_lines,
            self.total_lines,
            self.first_difference.unwrap_or(0),
            self.expected_line.as_deref().unwrap_or("<missing>"),
            self.actual_line.as_deref().unwrap_or("<missing>")
        )
    }
}

fn normalize(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::to_string)
        .collect()
}

pub fn compare_ddl(expected: &str, actual: &str) -> DdlComparison {
    let expected_lines = normalize(expected);
    let actual_lines = normalize(actual);
    let total = expected_lines.len().max(actual_lines.len());
    let mut differing = 0usize;
    let mut first_difference = None;
    let mut expected_line = None;
    let mut actual_line = None;
    for index in 0..total {
        let left = expected_lines.get(index);
        let right = actual_lines.get(index);
        if left != right {
            differing += 1;
            if first_difference.is_none() {
                first_difference = Some(index + 1);
                expected_line = left.cloned();
                actual_line = right.cloned();
            }
        }
    }
    DdlComparison {
        identical: differing == 0,
        total_lines: total,
        differing_lines: differing,
        first_difference,
        expected_line,
        actual_line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_normalizes_line_endings_and_bom() {
        let comparison = compare_ddl("\u{feff}a\r\nb\r\n", "a\nb\n");
        assert!(comparison.identical);
        assert_eq!(comparison.total_lines, 2);
    }

    #[test]
    fn reports_the_first_difference() {
        let comparison = compare_ddl("a\nb\nc", "a\nB\nc");
        assert!(!comparison.identical);
        assert_eq!(comparison.differing_lines, 1);
        assert_eq!(comparison.first_difference, Some(2));
        assert_eq!(comparison.expected_line.as_deref(), Some("b"));
        assert_eq!(comparison.actual_line.as_deref(), Some("B"));
        assert!(comparison.report().contains("line 2"));
    }

    #[test]
    fn detects_missing_lines() {
        let comparison = compare_ddl("a\nb", "a");
        assert!(!comparison.identical);
        assert_eq!(comparison.differing_lines, 1);
        assert_eq!(comparison.first_difference, Some(2));
        assert_eq!(comparison.actual_line, None);
    }
}
