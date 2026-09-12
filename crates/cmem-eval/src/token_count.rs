pub fn estimate_word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

pub const TOKENIZER_ENCODING: &str = "o200k_base";

pub fn count_tokens(text: &str) -> usize {
    tiktoken_rs::o200k_base_singleton()
        .encode_ordinary(text)
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_empty_text_as_zero_tokens() {
        assert_eq!(count_tokens(""), 0);
    }

    #[test]
    fn counts_ascii_text_with_o200k_base() {
        assert_eq!(count_tokens("hello world"), 2);
    }

    #[test]
    fn preserves_whitespace_in_tokenization() {
        assert!(count_tokens("hello      world") > count_tokens("hello world"));
    }

    #[test]
    fn counts_unicode_text_with_tokenizer() {
        assert_eq!(count_tokens("こんにちは世界"), 2);
        assert_eq!(count_tokens("hello 👋 world"), 4);
    }

    #[test]
    fn token_count_is_not_character_heuristic() {
        let text = "antidisestablishmentarianism";
        assert_eq!(count_tokens(text), 6);
        assert_ne!(count_tokens(text), text.chars().count().div_ceil(4));
    }

    #[test]
    fn exposes_tokenizer_encoding_name() {
        assert_eq!(TOKENIZER_ENCODING, "o200k_base");
    }
}
