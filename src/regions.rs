//! Utilities for checks that need to reason about marked regions of a file.

use std::fmt;

/// A pair of lines marking a protected region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionSpan {
    /// Index of the configured marker pair that produced this span.
    pub marker_index: usize,
    /// Line containing the start marker.
    pub start_line: usize,
    /// Line containing the end marker.
    pub end_line: usize,
}

impl RegionSpan {
    /// Returns whether `line` is within the marked region, including its
    /// boundary lines.
    pub(crate) fn contains(self, line: usize) -> bool {
        (self.start_line..=self.end_line).contains(&line)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionError {
    EndWithoutStart { marker_index: usize, line: usize },
    StartWithoutEnd { marker_index: usize, line: usize },
}

impl fmt::Display for RegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndWithoutStart {
                marker_index, line, ..
            } => write!(
                f,
                "region marker pair {marker_index} has an end marker on line {} without a start marker",
                line + 1
            ),
            Self::StartWithoutEnd {
                marker_index, line, ..
            } => write!(
                f,
                "region marker pair {marker_index} starts on line {} without an end marker",
                line + 1
            ),
        }
    }
}

/// Finds marked regions in `lines` using caller-provided marker matching.
///
/// Keeping matching outside this utility makes it usable with literal marker
/// strings, regular expressions, or another syntax-aware matcher without
/// making the region handling language-specific. Each marker pair is tracked
/// independently, so regions from different pairs may overlap.
pub(crate) fn find_region_spans<T>(
    lines: &[&str],
    markers: &[T],
    mut is_start: impl FnMut(&T, &str) -> bool,
    mut is_end: impl FnMut(&T, &str) -> bool,
) -> Result<Vec<RegionSpan>, RegionError> {
    let mut open = vec![None; markers.len()];
    let mut spans = Vec::new();

    for (line, content) in lines.iter().enumerate() {
        for (marker_index, marker) in markers.iter().enumerate() {
            let starts = is_start(marker, content);
            let ends = is_end(marker, content);

            match (open[marker_index], starts, ends) {
                (None, false, false) => {}
                (None, false, true) => {
                    return Err(RegionError::EndWithoutStart { marker_index, line });
                }
                (None, true, true) => {
                    spans.push(RegionSpan {
                        marker_index,
                        start_line: line,
                        end_line: line,
                    });
                }
                (None, true, false) => {
                    open[marker_index] = Some(line);
                }
                (Some(start_line), _, true) => {
                    spans.push(RegionSpan {
                        marker_index,
                        start_line,
                        end_line: line,
                    });
                    open[marker_index] = None;
                }
                (Some(_), false, false) => {}
                // A nested start marker for the same pair does not change the
                // active region. This matches the usual off/on convention.
                (Some(_), true, false) => {}
            }
        }
    }

    if let Some((marker_index, Some(line))) =
        open.iter().enumerate().find(|(_, line)| line.is_some())
    {
        return Err(RegionError::StartWithoutEnd {
            marker_index,
            line: *line,
        });
    }

    Ok(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_multiple_marker_pairs() {
        let lines = ["before", "BEGIN", "inside", "END", "after"];
        let markers = [("BEGIN", "END")];
        let spans = find_region_spans(
            &lines,
            &markers,
            |marker, line| line == marker.0,
            |marker, line| line == marker.1,
        )
        .unwrap();

        assert_eq!(
            spans,
            vec![RegionSpan {
                marker_index: 0,
                start_line: 1,
                end_line: 3,
            }]
        );
        assert!(spans[0].contains(2));
        assert!(!spans[0].contains(4));
    }

    #[test]
    fn reports_unbalanced_markers() {
        let lines = ["BEGIN", "inside"];
        let markers = [("BEGIN", "END")];
        let error = find_region_spans(
            &lines,
            &markers,
            |marker, line| line == marker.0,
            |marker, line| line == marker.1,
        )
        .unwrap_err();

        assert_eq!(
            error,
            RegionError::StartWithoutEnd {
                marker_index: 0,
                line: 0,
            }
        );
    }
}
