//! Wire positions are 0-based line and UTF-16 column; tree-sitter points are byte columns.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

fn line_start(text: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }
    let mut seen = 0u32;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            seen += 1;
            if seen == line {
                return Some(i + 1);
            }
        }
    }
    None
}

fn line_end(text: &str, start: usize) -> usize {
    match text[start..].find('\n') {
        Some(n) => start + n,
        None => text.len(),
    }
}

pub fn byte_offset(text: &str, p: Pos) -> Option<usize> {
    let start = line_start(text, p.line)?;
    let end = line_end(text, start);
    let mut units = 0u32;
    for (i, c) in text[start..end].char_indices() {
        if units == p.col {
            return Some(start + i);
        }
        units += c.len_utf16() as u32;
        if units > p.col {
            return None;
        }
    }
    if units == p.col { Some(end) } else { None }
}

pub fn pos_of(text: &str, byte: usize) -> Pos {
    let byte = byte.min(text.len());
    let mut line = 0u32;
    let mut start = 0usize;
    for (i, b) in text.as_bytes()[..byte].iter().enumerate() {
        if *b == b'\n' {
            line += 1;
            start = i + 1;
        }
    }
    let col = text[start..byte]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();
    Pos { line, col }
}

pub fn to_point(text: &str, p: Pos) -> Option<tree_sitter::Point> {
    let off = byte_offset(text, p)?;
    let start = line_start(text, p.line)?;
    Some(tree_sitter::Point {
        row: p.line as usize,
        column: off - start,
    })
}

pub fn from_point(text: &str, pt: tree_sitter::Point) -> Pos {
    let start = line_start(text, pt.row as u32).unwrap_or(text.len());
    let end = line_end(text, start);
    let col_end = (start + pt.column).min(end);
    Pos {
        line: pt.row as u32,
        col: text[start..col_end]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii() {
        let t = "ab\ncd";
        assert_eq!(byte_offset(t, Pos { line: 1, col: 1 }), Some(4));
        assert_eq!(pos_of(t, 4), Pos { line: 1, col: 1 });
    }

    #[test]
    fn utf16_surrogate_pair_counts_two() {
        let t = "𝕏 = 1";
        assert_eq!(byte_offset(t, Pos { line: 0, col: 2 }), Some(4));
        assert_eq!(pos_of(t, 4), Pos { line: 0, col: 2 });
    }

    #[test]
    fn out_of_range_is_none() {
        assert_eq!(byte_offset("a", Pos { line: 3, col: 0 }), None);
    }

    #[test]
    fn end_of_line_and_trailing_empty_line_are_valid() {
        let t = "ab\ncd\n";
        assert_eq!(byte_offset(t, Pos { line: 0, col: 2 }), Some(2));
        assert_eq!(byte_offset(t, Pos { line: 2, col: 0 }), Some(6));
        assert_eq!(byte_offset(t, Pos { line: 0, col: 3 }), None);
    }

    #[test]
    fn column_inside_a_multi_unit_char_is_none() {
        assert_eq!(byte_offset("𝕏x", Pos { line: 0, col: 1 }), None);
    }

    #[test]
    fn points_use_byte_columns() {
        let t = "𝕏 = 1\nok";
        assert_eq!(
            to_point(t, Pos { line: 0, col: 2 }),
            Some(tree_sitter::Point { row: 0, column: 4 })
        );
        assert_eq!(
            from_point(t, tree_sitter::Point { row: 0, column: 4 }),
            Pos { line: 0, col: 2 }
        );
        assert_eq!(
            from_point(t, tree_sitter::Point { row: 1, column: 1 }),
            Pos { line: 1, col: 1 }
        );
    }
}
