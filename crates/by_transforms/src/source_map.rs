/// Build an output-line → input-line table for a single edit-application pass.
///
/// `edits` must be ascending by start and non-overlapping, expressed in `source`
/// byte coordinates (the same shape `replace_range` is fed). Each output line
/// (counted by `\n`) maps to the input line it came from, or `None` when it was
/// produced by an edit's replacement text. This is the line-level primitive the
/// run-time traceback rewriter composes; column-accurate mapping is future work
/// (see `docs/basedpython/development/sourcemaps.md`).
pub fn line_table(source: &str, edits: &[(usize, usize, String)]) -> Vec<Option<u32>> {
    let mut lines: Vec<Option<u32>> = Vec::new();
    let mut src_pos = 0usize;
    let mut input_line = 0u32;

    for (start, end, new_text) in edits {
        for ch in source[src_pos..*start].chars() {
            if ch == '\n' {
                lines.push(Some(input_line));
                input_line += 1;
            }
        }
        let consumed = source[*start..*end].chars().filter(|&c| c == '\n').count();
        for ch in new_text.chars() {
            if ch == '\n' {
                lines.push(Some(input_line));
            }
        }
        input_line += u32::try_from(consumed).unwrap_or(0);
        src_pos = *end;
    }
    for ch in source[src_pos..].chars() {
        if ch == '\n' {
            lines.push(Some(input_line));
            input_line += 1;
        }
    }
    lines
}
