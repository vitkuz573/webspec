use super::DriftError;
use scraper::{Html, Selector};

pub fn selector_exists(html: &str, selector: &str) -> Result<bool, DriftError> {
    let document = Html::parse_document(html);
    let sel = Selector::parse(selector)
        .map_err(|_| DriftError::InvalidSelector(selector.to_string()))?;
    Ok(document.select(&sel).next().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"<!DOCTYPE html>
<html><body>
  <div class="item-title">Present</div>
</body></html>"#;

    #[test]
    fn existing_selector_returns_true() {
        assert!(selector_exists(HTML, ".item-title").unwrap());
    }

    #[test]
    fn missing_selector_returns_false() {
        assert!(!selector_exists(HTML, ".missing-price").unwrap());
    }

    #[test]
    fn invalid_selector_returns_error() {
        assert!(selector_exists(HTML, "<<<").is_err());
    }
}
