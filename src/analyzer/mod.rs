pub mod html;
pub mod patterns;
pub mod attributes;

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSelectorData {
    pub selector: String,
    pub count: usize,
    pub sample_values: Vec<String>,
    pub sample_attributes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPageData {
    pub url: String,
    pub titles: Vec<String>,
    pub selectors: Vec<RawSelectorData>,
    pub data_attributes: Vec<attributes::DataAttribute>,
    pub url_patterns: Vec<UrlPattern>,
    pub pages_crawled: Vec<CrawledPage>,
    pub internal_urls: Vec<UrlWithContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlWithContext {
    pub url: String,
    pub context: String,
    pub anchor_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlPattern {
    pub pattern: String,
    pub samples: Vec<String>,
    pub parameters: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawledPage {
    pub url: String,
    pub title: String,
}

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

pub async fn collect_raw_data(url: &str, depth: u32, max_pages: usize) -> anyhow::Result<RawPageData> {
    let client = build_client();
    let base_url = extract_base_url(url);

    let html = fetch_page(&client, url).await?;
    let clean_html = html::strip_noise(&html);
    let document = Html::parse_document(&clean_html);

    let title = extract_page_title(&document);

    let (internal_links, nav_links) = discover_internal_links(&document, &base_url);

    let mut prioritized_links = nav_links.clone();
    for link in &internal_links {
        if !prioritized_links.contains(link) {
            prioritized_links.push(link.clone());
        }
    }
    let pages_to_crawl = select_pages_to_crawl(&prioritized_links, url, max_pages.min(15));

    let mut all_data_attrs = attributes::extract_data_attributes(&clean_html);
    let mut all_htmls: Vec<(String, String)> = vec![(url.to_string(), clean_html.clone())];
    let mut crawled_pages: Vec<CrawledPage> = Vec::new();
    let mut crawled_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    crawled_urls.insert(url.to_string());
    let mut titles: Vec<String> = vec![title];

    let mut current_page_urls = pages_to_crawl.clone();
    let mut next_depth_urls: Vec<String> = Vec::new();

    for current_depth in 0..depth {
        let urls_at_depth = if current_depth == 0 {
            current_page_urls.clone()
        } else {
            std::mem::take(&mut next_depth_urls)
        };

        if urls_at_depth.is_empty() {
            break;
        }

        for page_url in &urls_at_depth {
            if page_url == url || crawled_urls.contains(page_url) {
                continue;
            }
            if crawled_urls.len() >= max_pages {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

            match fetch_page(&client, page_url).await {
                Ok(page_html) => {
                    crawled_urls.insert(page_url.clone());
                    let clean = html::strip_noise(&page_html);
                    let page_doc = Html::parse_document(&clean);
                    let page_title = extract_page_title(&page_doc);
                    if !page_title.is_empty() {
                        titles.push(page_title.clone());
                    }
                    let page_attrs = attributes::extract_data_attributes(&clean);
                    all_data_attrs.extend(page_attrs);
                    all_htmls.push((page_url.clone(), clean.clone()));
                    crawled_pages.push(CrawledPage {
                        url: page_url.clone(),
                        title: page_title,
                    });

                    if current_depth + 1 < depth {
                        let (page_links, page_nav_links) = discover_internal_links(&page_doc, &base_url);
                        let mut new_links: Vec<String> = page_links.into_iter()
                            .filter(|link| !crawled_urls.contains(link) && !current_page_urls.contains(link))
                            .collect();
                        for nav_link in &page_nav_links {
                            if !new_links.contains(nav_link) && !crawled_urls.contains(nav_link) && !current_page_urls.contains(nav_link) {
                                new_links.insert(0, nav_link.clone());
                            }
                        }
                        next_depth_urls.extend(new_links);
                    }
                }
                Err(e) => {
                    eprintln!("  Failed to fetch {}: {}", page_url, e);
                }
            }
        }

        current_page_urls = next_depth_urls.clone();
        next_depth_urls.clear();
    }

    let mut all_url_patterns = detect_url_patterns(&document);
    for (_, html_content) in &all_htmls[1..] {
        let doc = Html::parse_document(html_content);
        let mut patterns = detect_url_patterns(&doc);
        all_url_patterns.append(&mut patterns);
    }
    all_url_patterns.dedup_by(|a, b| a.pattern == b.pattern);

    let selectors = extract_selectors_from_all(&all_htmls);

    let internal_urls = collect_internal_urls(&document, &base_url, url);

    Ok(RawPageData {
        url: url.to_string(),
        titles,
        selectors,
        data_attributes: all_data_attrs,
        url_patterns: all_url_patterns,
        pages_crawled: crawled_pages,
        internal_urls,
    })
}

fn extract_selectors_from_all(htmls: &[(String, String)]) -> Vec<RawSelectorData> {
    let mut selector_map: HashMap<String, (usize, Vec<String>, Vec<String>)> = HashMap::new();

    for (_url, html) in htmls {
        let doc = Html::parse_document(html);
        extract_selectors_from_doc(&doc, &mut selector_map);
    }

    let mut result: Vec<RawSelectorData> = selector_map.into_iter()
        .filter(|(_, (count, _, _))| *count >= 3)
        .map(|(selector, (count, values, attrs))| RawSelectorData {
            selector,
            count,
            sample_values: values.into_iter().take(5).collect(),
            sample_attributes: attrs.into_iter().take(5).collect(),
        })
        .collect();

    result.sort_by(|a, b| b.count.cmp(&a.count));
    result.truncate(100);
    result
}

fn extract_selectors_from_doc(
    doc: &Html,
    map: &mut HashMap<String, (usize, Vec<String>, Vec<String>)>,
) {
    for el in doc
        .root_element()
        .descendants()
        .filter_map(scraper::ElementRef::wrap)
    {
        let classes: Vec<&str> = el.value().classes().collect();
        if classes.is_empty() {
            continue;
        }

        let selector = format!("{}.{}", el.value().name(), classes.join("."));
        let text: String = el.text().collect::<Vec<&str>>().join(" ").trim().to_string();
        let data_attrs: Vec<String> = el.value().attrs()
            .filter(|(k, _)| k.starts_with("data-"))
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let entry = map.entry(selector).or_insert((0, Vec::new(), Vec::new()));
        entry.0 += 1;
        if !text.is_empty() && entry.1.len() < 5 {
            entry.1.push(text);
        }
        if !data_attrs.is_empty() && entry.2.len() < 5 {
            entry.2.extend(data_attrs);
        }
    }
}

fn collect_internal_urls(document: &Html, base_url: &str, main_url: &str) -> Vec<UrlWithContext> {
    let sel = Selector::parse("a[href]").unwrap();
    let host = base_url.trim_start_matches("https://").trim_start_matches("http://");

    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();

    for el in document.select(&sel) {
        if let Some(href) = el.value().attr("href") {
            if let Some(url) = normalize_href(href, base_url, host) {
                if url != main_url && !seen.contains(&url) {
                    seen.insert(url.clone());
                    let anchor: String = el.text().collect::<Vec<&str>>().join(" ").trim().to_string();
                    let context = infer_link_context(el);
                    urls.push(UrlWithContext {
                        url,
                        context,
                        anchor_text: anchor,
                    });
                }
            }
        }
    }

    urls
}

fn infer_link_context(el: scraper::ElementRef) -> String {
    let mut current = el.parent();
    while let Some(parent) = current {
        if let Some(parent_el) = scraper::ElementRef::wrap(parent) {
            let tag = parent_el.value().name();
            if tag == "nav" || tag == "header" || tag == "aside" {
                return format!("navigation/{}", tag);
            }
            let classes: Vec<&str> = parent_el.value().classes().collect();
            let class_str = classes.join(" ");
            if class_str.contains("nav") || class_str.contains("menu") || class_str.contains("sidebar") {
                return format!("navigation/{}", class_str);
            }
            if class_str.contains("list") || class_str.contains("grid") || class_str.contains("catalog") {
                return format!("listing/{}", class_str);
            }
        }
        current = parent.parent();
    }
    "main content".to_string()
}

async fn fetch_page(client: &reqwest::Client, url: &str) -> anyhow::Result<String> {
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp.text().await?)
}

fn extract_base_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""))
    } else {
        let parts: Vec<&str> = url.splitn(3, '/').collect();
        if parts.len() >= 3 {
            format!("{}://{}", parts[0].trim_end_matches(':'), parts[1])
        } else {
            url.to_string()
        }
    }
}

fn discover_internal_links(document: &Html, base_url: &str) -> (Vec<String>, Vec<String>) {
    let sel = Selector::parse("a[href]").unwrap();
    let host = base_url.trim_start_matches("https://").trim_start_matches("http://");

    let nav_selectors = ["nav a[href]", "header a[href]", ".sidebar a[href]", "[role='navigation'] a[href]"];
    let mut nav_links_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for nav_sel in &nav_selectors {
        if let Ok(sel) = Selector::parse(nav_sel) {
            for el in document.select(&sel) {
                if let Some(href) = el.value().attr("href") {
                    if let Some(url) = normalize_href(href, base_url, host) {
                        nav_links_set.insert(url);
                    }
                }
            }
        }
    }

    let mut all_links: Vec<String> = Vec::new();
    for el in document.select(&sel) {
        if let Some(href) = el.value().attr("href") {
            if let Some(url) = normalize_href(href, base_url, host) {
                all_links.push(url);
            }
        }
    }

    let nav_links: Vec<String> = nav_links_set.into_iter().collect();
    (all_links, nav_links)
}

fn normalize_href(href: &str, base_url: &str, host: &str) -> Option<String> {
    if href.starts_with('/') {
        Some(format!("{}{}", base_url, href))
    } else if href.starts_with("http") && href.contains(host) {
        Some(href.to_string())
    } else {
        None
    }
}

fn select_pages_to_crawl(links: &[String], main_url: &str, max: usize) -> Vec<String> {
    let mut selected: Vec<String> = Vec::new();
    let mut seen_segments: std::collections::HashSet<String> = std::collections::HashSet::new();

    selected.push(main_url.to_string());
    for seg in path_segments(main_url) {
        seen_segments.insert(seg);
    }

    let mut scored_links: Vec<(String, f64)> = links
        .iter()
        .filter(|link| {
            let path = extract_path(link);
            path != "/"
                && !path.is_empty()
                && !path.starts_with("/auth")
                && !path.starts_with("/login")
                && !path.starts_with("/register")
                && !path.starts_with("/signup")
                && !path.ends_with(".js")
                && !path.ends_with(".css")
                && !path.ends_with(".png")
                && !path.ends_with(".jpg")
                && !path.ends_with(".jpeg")
                && !path.ends_with(".ico")
                && !path.ends_with(".gif")
                && !path.ends_with(".svg")
                && !path.ends_with(".woff")
                && !path.ends_with(".woff2")
                && !path.ends_with(".ttf")
        })
        .map(|link| {
            let path = extract_path(link);
            let diversity_score = score_diversity(&path, &seen_segments);
            let base_priority = score_url_priority(&path) as f64;
            (link.clone(), base_priority + diversity_score)
        })
        .collect();

    scored_links.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (link, _score) in scored_links {
        if selected.len() >= max {
            break;
        }
        let link_path = extract_path(&link);
        if selected.iter().any(|s| extract_path(s) == link_path) {
            continue;
        }
        selected.push(link.clone());
        for seg in path_segments(&link) {
            seen_segments.insert(seg);
        }
    }

    selected
}

fn path_segments(url: &str) -> Vec<String> {
    let path = extract_path(url);
    path.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let clean = s.split('?').next().unwrap_or(s);
            if clean.chars().all(|c| c.is_ascii_digit()) {
                "{id}".to_string()
            } else {
                clean.to_string()
            }
        })
        .collect()
}

fn score_diversity(path: &str, seen: &std::collections::HashSet<String>) -> f64 {
    let segments: Vec<String> = path.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let clean = s.split('?').next().unwrap_or(s);
            if clean.chars().all(|c| c.is_ascii_digit()) {
                "{id}".to_string()
            } else {
                clean.to_string()
            }
        })
        .collect();

    if segments.is_empty() {
        return 0.0;
    }

    let new_count = segments.iter().filter(|s| !seen.contains(s.as_str())).count();
    let seen_count = segments.len() - new_count;
    (new_count as f64) * 15.0 - (seen_count as f64) * 5.0
}

fn is_low_value_url(path: &str) -> bool {
    let lower = path.to_lowercase();
    if lower.starts_with("/account")
        || lower.starts_with("/pages")
        || lower.starts_with("/trade/info")
        || lower.starts_with("/support")
        || lower.starts_with("/settings")
        || lower.starts_with("/profile")
        || lower.starts_with("/auth")
    {
        return true;
    }
    let skip_keywords = ["login", "register", "signup", "sign-up", "password", "forgot",
                         "privacy", "cookie", "terms", "about", "contact", "faq",
                         "help", "legal", "refund", "sitemap"];
    skip_keywords.iter().any(|kw| lower.contains(kw))
}

fn score_url_priority(path: &str) -> i32 {
    if is_low_value_url(path) {
        return -100;
    }

    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let depth = parts.len();

    let has_numeric_segment = parts.iter().any(|p| {
        p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()
    });

    if has_numeric_segment && depth >= 2 {
        return 90;
    }

    if depth == 2 && !has_numeric_segment { return 85; }
    if depth == 1 && !parts.is_empty() && parts[0] != "" { return 75; }
    if depth >= 3 && !has_numeric_segment { return 70; }

    10
}

fn extract_path(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path().to_string();
        let query = parsed.query().map(|q| format!("?{}", q)).unwrap_or_default();
        format!("{}{}", path, query)
    } else {
        url.splitn(2, '?').nth(0).unwrap_or("/").to_string()
    }
}

fn extract_page_title(document: &Html) -> String {
    let sel = Selector::parse("title").unwrap();
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default()
}

fn detect_url_patterns(document: &Html) -> Vec<UrlPattern> {
    let sel = Selector::parse("a[href]").unwrap();
    let urls: Vec<String> = document
        .select(&sel)
        .filter_map(|el| el.value().attr("href"))
        .filter(|h| h.starts_with('/'))
        .map(|h| h.to_string())
        .collect();

    if urls.len() < 3 {
        return Vec::new();
    }

    let mut url_groups: HashMap<String, Vec<String>> = HashMap::new();
    for url in &urls {
        let parts: Vec<&str> = url.trim_start_matches('/').split('/').collect();
        if parts.len() >= 2 {
            let prefix = parts[0..2].join("/");
            url_groups.entry(prefix).or_default().push(url.clone());
        }
    }

    let mut patterns = Vec::new();
    for (_prefix, group_urls) in url_groups {
        if group_urls.len() < 2 {
            continue;
        }

        if let Some(pattern) = patterns::extract_url_pattern(&group_urls) {
            let params = extract_params_from_pattern(&pattern);
            patterns.push(UrlPattern {
                pattern,
                samples: group_urls.into_iter().take(5).collect(),
                parameters: params,
            });
        }
    }

    patterns.sort_by(|a, b| b.samples.len().cmp(&a.samples.len()));
    patterns
}

fn extract_params_from_pattern(pattern: &str) -> Vec<String> {
    let mut params = Vec::new();
    for part in pattern.split('/') {
        if part.starts_with('{') && part.ends_with('}') {
            params.push(part[1..part.len() - 1].to_string());
        }
    }
    params
}
