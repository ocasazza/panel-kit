//! Scroll policy and text wrapping math.

/// The maximum line offset for `total` lines in a `view`-row viewport.
pub fn max_offset(total: usize, view: u16) -> usize {
    total.saturating_sub(view as usize)
}

/// Hard-wrap `text` into lines of at most `width` characters.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if width == 0 || chars.len() <= width {
        return vec![text.to_string()];
    }

    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_offset_matches_tui_saturating_math() {
        assert_eq!(max_offset(10, 4), 6);
        assert_eq!(max_offset(4, 4), 0);
        assert_eq!(max_offset(2, 4), 0);
    }

    #[test]
    fn wrap_matches_tui_character_chunks() {
        assert_eq!(wrap("abcdef", 2), vec!["ab", "cd", "ef"]);
        assert_eq!(wrap("abc", 0), vec!["abc"]);
        assert_eq!(wrap("abc", 5), vec!["abc"]);
    }
}
