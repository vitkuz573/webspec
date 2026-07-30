pub mod check;
pub mod fetch;
pub mod report;

use crate::spec::{ApiSpec, DriftPage};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftError {
    InvalidSelector(String),
    InvalidUrl(String),
    BlockedScheme(String),
    FetchFailed(String),
}

impl std::fmt::Display for DriftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftError::InvalidSelector(s) => write!(f, "invalid selector: {s}"),
            DriftError::InvalidUrl(s) => write!(f, "invalid URL: {s}"),
            DriftError::BlockedScheme(s) => write!(f, "blocked URL scheme: {s}"),
            DriftError::FetchFailed(s) => write!(f, "fetch failed: {s}"),
        }
    }
}

impl std::error::Error for DriftError {}

#[derive(Debug, Clone, Default)]
pub struct DriftFailure {
    pub url: String,
    pub selector: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DriftReport {
    pub checked: Vec<(String, String)>,
    pub missing: Vec<DriftFailure>,
    pub errors: Vec<(String, String)>,
    pub drifted: bool,
}

pub async fn run_drift(
    spec: &ApiSpec,
    client: &reqwest::Client,
    dry_run: bool,
) -> Result<DriftReport, DriftError> {
    let mut report = DriftReport::default();

    let base = spec.base_url.clone().unwrap_or_default();
    if !base.is_empty() {
        validate_url_scheme(&base)?;
    }

    let detection = spec.drift_detection.clone().unwrap_or_default();
    let pages = detection.pages.unwrap_or_default();
    let _critical = detection.critical_selectors.unwrap_or_default();

    let rps = spec
        .rate_limits
        .as_ref()
        .and_then(|r| r.requests_per_second)
        .unwrap_or(10.0)
        .max(0.1);
    let min_interval = Duration::from_secs_f64(1.0 / rps);
    let mut last_request = Option::<Instant>::None;

    let targets = collect_targets(spec, &pages)?;

    for (_name, url, selectors) in targets {
        if dry_run {
            println!("{url}");
            continue;
        }

        if let Some(last) = last_request {
            let elapsed = last.elapsed();
            if elapsed < min_interval {
                tokio::time::sleep(min_interval - elapsed).await;
            }
        }

        match fetch::fetch_page(client, &url, Duration::from_secs(15)).await {
            Ok(html) => {
                last_request = Some(Instant::now());
                for (sel, context) in selectors {
                    report.checked.push((url.clone(), sel.clone()));
                    match check::selector_exists(&html, &sel) {
                        Ok(true) => {}
                        Ok(false) => {
                            report.missing.push(DriftFailure {
                                url: url.clone(),
                                selector: sel,
                                context,
                            });
                            report.drifted = true;
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            report.errors.push((url.clone(), msg));
                        }
                    }
                }
            }
            Err(e) => {
                report.errors.push((url.clone(), e.to_string()));
            }
        }
    }

    Ok(report)
}

fn collect_targets(
    spec: &ApiSpec,
    pages: &BTreeMap<String, DriftPage>,
) -> Result<Vec<(String, String, Vec<(String, Option<String>)>)>, DriftError> {
    let base = spec.base_url.clone().unwrap_or_default();
    let mut targets: Vec<(String, String, Vec<(String, Option<String>)>)> = Vec::new();

    for (name, page) in pages {
        let url = build_absolute_url(&base, &page.url)?;
        let mut selectors: Vec<(String, Option<String>)> = page
            .selectors
            .iter()
            .map(|(k, v)| (v.clone(), Some(k.clone())))
            .collect();

        if let Some(detection) = &spec.drift_detection {
            if let Some(critical) = &detection.critical_selectors {
                for cs in critical {
                    selectors.push((cs.selector.clone(), Some(cs.context.clone())));
                }
            }
        }

        if !selectors.is_empty() {
            targets.push((name.clone(), url, selectors));
        }
    }

    Ok(targets)
}

pub fn build_absolute_url(base: &str, path: &str) -> Result<String, DriftError> {
    if path.starts_with("http://") || path.starts_with("https://") {
        validate_url_scheme(path)?;
        return Ok(path.to_string());
    }

    if path.starts_with("file://") {
        return Err(DriftError::BlockedScheme("file://".to_string()));
    }

    if base.is_empty() {
        return Err(DriftError::InvalidUrl(format!(
            "cannot resolve relative path '{path}' without base_url"
        )));
    }

    validate_url_scheme(base)?;

    let base_trim = base.trim_end_matches('/');
    let path_trim = path.trim_start_matches('/');
    Ok(format!("{base_trim}/{path_trim}"))
}

fn validate_url_scheme(url: &str) -> Result<(), DriftError> {
    if url.starts_with("file://") {
        return Err(DriftError::BlockedScheme("file://".to_string()));
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(DriftError::InvalidUrl(format!(
            "URL scheme must be http or https: {url}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_spec() -> ApiSpec {
        ApiSpec {
            version: "1.0.0".to_string(),
            protocol: "webspec".to_string(),
            name: "Sample".to_string(),
            base_url: Some("https://example.com".to_string()),
            info: None,
            types: BTreeMap::new(),
            enums: BTreeMap::new(),
            entities: BTreeMap::new(),
            pages: BTreeMap::new(),
            auth: None,
            rate_limits: Some(crate::spec::RateLimitsDef {
                requests_per_second: Some(10.0),
                max_retries: Some(0),
            }),
            drift_detection: None,
        }
    }

    #[test]
    fn test_build_absolute_url_rejects_file_scheme() {
        let err = build_absolute_url("https://example.com", "file:///etc/passwd").unwrap_err();
        assert!(matches!(err, DriftError::BlockedScheme(_)));
    }

    #[test]
    fn test_build_absolute_url_rejects_invalid_scheme() {
        let err = build_absolute_url("ftp://example.com", "/items").unwrap_err();
        assert!(matches!(err, DriftError::InvalidUrl(_)));
    }

    #[test]
    fn test_build_absolute_url_joins_relative() {
        let url = build_absolute_url("https://example.com/", "/items").unwrap();
        assert_eq!(url, "https://example.com/items");
    }
}
