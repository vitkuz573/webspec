use scraper::ElementRef;

/// Strip noise from HTML: scripts, styles, comments, excess whitespace
pub fn strip_noise(html: &str) -> String {
    let html = regex::Regex::new(r"<!--.*?-->")
        .unwrap()
        .replace_all(html, "");
    let html = regex::Regex::new(r"<script[^>]*>.*?</script>")
        .unwrap()
        .replace_all(&html, "");
    let html = regex::Regex::new(r"<style[^>]*>.*?</style>")
        .unwrap()
        .replace_all(&html, "");
    let html = regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(&html, " ");
    html.to_string()
}

/// Convert HTML element to annotated text representation
pub fn annotate_element(el: ElementRef, depth: usize) -> String {
    let mut result = String::new();
    let indent = "  ".repeat(depth);

    let tag = el.value().name();
    let classes: Vec<&str> = el.value().classes().collect();
    let data_attrs: Vec<(String, String)> = el
        .value()
        .attrs()
        .filter(|(k, _)| k.starts_with("data-"))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let mut selector_hint = tag.to_string();
    if !classes.is_empty() {
        selector_hint.push_str(&format!(".{}", classes.join(".")));
    }

    let attr_str = if data_attrs.is_empty() {
        String::new()
    } else {
        let pairs: Vec<String> = data_attrs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        format!(" [{}]", pairs.join(", "))
    };

    let text: String = el.text().collect::<Vec<&str>>().join(" ").trim().to_string();

    if !text.is_empty() || !data_attrs.is_empty() {
        result.push_str(&format!(
            "{}{}:{} \"{}\"",
            indent, selector_hint, attr_str, text
        ));
        result.push('\n');
    }

    for child in el.children() {
        if let Some(child_ref) = ElementRef::wrap(child) {
            result.push_str(&annotate_element(child_ref, depth + 1));
        }
    }

    result
}
