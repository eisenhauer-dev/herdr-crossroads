pub(crate) fn matches_field(field: &str, query: &str) -> bool {
    let mut chars = field.to_lowercase().chars().collect::<Vec<_>>().into_iter();
    query
        .to_lowercase()
        .chars()
        .all(|c| chars.any(|next| next == c))
}

pub(crate) fn match_positions(field: &str, query: &str) -> Vec<usize> {
    if query.is_empty() || !matches_field(field, query) {
        return Vec::new();
    }
    let lowered = field.to_lowercase();
    let query = query.to_lowercase();
    let mut wanted = query.chars().peekable();
    let mut positions = Vec::new();
    let mut source = field
        .chars()
        .enumerate()
        .flat_map(|(index, c)| std::iter::repeat_n(index, c.to_lowercase().count()));
    for lower in lowered.chars() {
        let index = source.next().unwrap();
        if wanted.peek() == Some(&lower) {
            wanted.next();
            if positions.last() != Some(&index) {
                positions.push(index);
            }
        }
        if wanted.peek().is_none() {
            break;
        }
    }
    positions
}
