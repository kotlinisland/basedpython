use ruff_text_size::TextRange;

/// maps positions in the transpiled python output back to the original `.by` source
#[derive(Clone)]
pub struct SourceMap {
    /// `lines[i] = Some(j)`: output line `i` (0-indexed) came from input line `j` (0-indexed)
    /// `lines[i] = None`: output line `i` is generated (preamble or edit-inserted)
    lines: Vec<Option<u32>>,
}

impl SourceMap {
    /// build a source map from the deduplicated body edits (ascending, no overlaps)
    /// and the preamble string that was prepended to the body
    pub fn build(source: &str, body_edits: &[(TextRange, String)], preamble: &str) -> Self {
        let preamble_line_count = preamble.chars().filter(|&c| c == '\n').count();

        let mut body_lines: Vec<Option<u32>> = Vec::new();
        let mut src_pos = 0usize;
        let mut input_line = 0u32;

        for (range, new_text) in body_edits {
            let start = usize::from(range.start());
            let end = usize::from(range.end());

            // unchanged region before this edit
            for ch in source[src_pos..start].chars() {
                if ch == '\n' {
                    body_lines.push(Some(input_line));
                    input_line += 1;
                }
            }

            // lines consumed by this edit in the source
            let input_lines_consumed =
                source[start..end].chars().filter(|&c| c == '\n').count() as u32;

            // lines produced by new_text — all approximate to the edit's start line
            for ch in new_text.chars() {
                if ch == '\n' {
                    body_lines.push(Some(input_line));
                }
            }

            input_line += input_lines_consumed;
            src_pos = end;
        }

        // remaining unchanged region
        for ch in source[src_pos..].chars() {
            if ch == '\n' {
                body_lines.push(Some(input_line));
                input_line += 1;
            }
        }

        let mut lines = vec![None; preamble_line_count];
        lines.extend(body_lines);

        Self { lines }
    }

    /// map a 1-indexed python `(row, col)` to a 1-indexed `.by` `(row, col)`
    /// returns `None` if the position is in the generated preamble
    pub fn py_to_by(&self, py_row: u32, py_col: u32) -> Option<(u32, u32)> {
        let output_line = py_row.checked_sub(1)? as usize;
        let input_line = self.lines.get(output_line)?.as_ref().copied()?;
        Some((input_line + 1, py_col))
    }

    /// map a 1-indexed `.by` `(row, col)` to a 1-indexed python `(row, col)`
    pub fn by_to_py(&self, by_row: u32, by_col: u32) -> (u32, u32) {
        let target = by_row.saturating_sub(1);
        for (output_line, &input_line) in self.lines.iter().enumerate() {
            if input_line == Some(target) {
                return (output_line as u32 + 1, by_col);
            }
        }
        // fallback: offset by preamble
        let preamble_lines = self.preamble_lines();
        (by_row + preamble_lines, by_col)
    }

    /// number of generated preamble lines at the start of the output
    pub fn preamble_lines(&self) -> u32 {
        self.lines.iter().take_while(|&&l| l.is_none()).count() as u32
    }
}
