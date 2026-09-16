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
    fn preserves_whitespace_in_tokenization() {
        assert!(count_tokens("hello      world") > count_tokens("hello world"));
    }

    #[test]
    fn token_count_is_not_character_heuristic() {
        let text = "antidisestablishmentarianism";
        assert_eq!(count_tokens(text), 6);
        assert_ne!(count_tokens(text), text.chars().count().div_ceil(4));
    }
}
