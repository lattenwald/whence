//! Wire positions are 0-based line and UTF-16 column; tree-sitter points are byte columns.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

/// Byte offsets of every line start, so position lookups are a binary search, not a scan.
pub struct Lines(Vec<usize>);

impl Lines {
    pub fn new(text: &str) -> Lines {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Lines(starts)
    }

    pub fn start(&self, line: u32) -> Option<usize> {
        self.0.get(line as usize).copied()
    }

    fn line_at(&self, byte: usize) -> (u32, usize) {
        let line = self.0.partition_point(|&s| s <= byte) - 1;
        (line as u32, self.0[line])
    }

    pub fn byte_offset(&self, text: &str, p: Pos) -> Option<usize> {
        let start = self.start(p.line)?;
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

    pub fn pos_of(&self, text: &str, byte: usize) -> Pos {
        let byte = byte.min(text.len());
        let (line, start) = self.line_at(byte);
        let col = text[start..byte]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum();
        Pos { line, col }
    }

    pub fn line_text<'t>(&self, text: &'t str, byte: usize) -> &'t str {
        let (_, start) = self.line_at(byte.min(text.len()));
        text[start..line_end(text, start)].trim()
    }
}

fn line_end(text: &str, start: usize) -> usize {
    match text[start..].find('\n') {
        Some(n) => start + n,
        None => text.len(),
    }
}

pub fn byte_offset(text: &str, p: Pos) -> Option<usize> {
    Lines::new(text).byte_offset(text, p)
}

pub fn pos_of(text: &str, byte: usize) -> Pos {
    Lines::new(text).pos_of(text, byte)
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
}
