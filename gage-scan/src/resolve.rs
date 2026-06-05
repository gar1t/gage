/// First non-empty line of `text`, with leading whitespace and any
/// markdown-heading marker (`#+\s+`) stripped. Empty string if `text`
/// has no non-empty lines.
pub fn first_line(text: &str) -> &str {
    let Some(line) = text.lines().find(|l| !l.trim().is_empty()) else {
        return "";
    };
    let trimmed = line.trim_start();
    let after_hashes = trimmed.trim_start_matches('#');
    let hash_count = trimmed.len() - after_hashes.len();
    if hash_count > 0 && after_hashes.starts_with(|c: char| c.is_whitespace()) {
        after_hashes.trim_start()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_strips_heading_marker() {
        assert_eq!(first_line("# Enable thinking\n\nbody"), "Enable thinking");
        assert_eq!(first_line("### Triple\nbody"), "Triple");
    }

    #[test]
    fn first_line_passes_plain_through() {
        assert_eq!(first_line("Plain summary\nrest"), "Plain summary");
    }

    #[test]
    fn first_line_skips_leading_blanks() {
        assert_eq!(first_line("\n\n# Heading"), "Heading");
    }

    #[test]
    fn first_line_does_not_strip_hash_without_space() {
        assert_eq!(first_line("##hashtag"), "##hashtag");
    }

    #[test]
    fn first_line_empty_when_no_content() {
        assert_eq!(first_line(""), "");
        assert_eq!(first_line("\n\n   \n"), "");
    }
}
