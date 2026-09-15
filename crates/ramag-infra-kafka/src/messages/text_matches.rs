fn text_matches(bytes: &[u8], matcher: &MessageSearchMatcher) -> bool {
    let text = String::from_utf8_lossy(bytes);
    matcher.regex.as_ref().map_or_else(
        || text.to_lowercase().contains(&matcher.query),
        |regex| regex.is_match(&text),
    )
}
