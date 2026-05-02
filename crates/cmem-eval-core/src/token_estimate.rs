pub fn estimate_word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn estimate_token_count(text: &str) -> usize {
    text.chars()
        .count()
        .div_ceil(4)
        .max(estimate_word_count(text))
}
