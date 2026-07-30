use super::DriftError;
use std::time::Duration;

pub async fn fetch_page(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
) -> Result<String, DriftError> {
    if url.starts_with("file://") {
        return Err(DriftError::BlockedScheme("file://".to_string()));
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(DriftError::InvalidUrl(format!(
            "URL scheme must be http or https: {url}"
        )));
    }

    let text = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| DriftError::FetchFailed(e.to_string()))?
        .error_for_status()
        .map_err(|e| DriftError::FetchFailed(e.to_string()))?
        .text()
        .await
        .map_err(|e| DriftError::FetchFailed(e.to_string()))?;

    Ok(text)
}
