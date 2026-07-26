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
    let cache_dir = std::path::PathBuf::from("/tmp/webspec_raw_cache");
    let cache_key = format!("{}_{}_{}", url.replace('/', "_").replace(':', "_"), depth, max_pages);
    let cache_path = cache_dir.join(format!("{}.json", cache_key));

    if cache_path.exists() {
        if let Ok(cached) = std::fs::read_to_string(&cache_path) {
            if let Ok(raw_data) = serde_json::from_str::<RawPageData>(&cached) {
                eprintln!("  Raw data cache hit for {}", url);
                return Ok(raw_data);
            }
        }
    }

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
    let pages_to_crawl = select_pages_to_crawl(&prioritized_links, url, max_pages);

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
    all_url_patterns.sort_by(|a, b| {
        a.pattern.cmp(&b.pattern)
            .then(a.samples.cmp(&b.samples))
    });
    all_url_patterns.dedup_by(|a, b| a.pattern == b.pattern);

    let selectors = extract_selectors_from_all(&all_htmls);

    let mut internal_urls = collect_internal_urls(&document, &base_url, url);
    internal_urls.sort_by(|a, b| a.url.cmp(&b.url));

    let mut sorted_crawled = crawled_pages;
    sorted_crawled.sort_by(|a, b| a.url.cmp(&b.url));

    let mut sorted_titles = titles;
    sorted_titles.sort();

    all_data_attrs.sort_by(|a, b| {
        a.element_tag.cmp(&b.element_tag)
            .then(a.attribute_name.cmp(&b.attribute_name))
            .then(a.value.cmp(&b.value))
    });

    let result = Ok(RawPageData {
        url: url.to_string(),
        titles: sorted_titles,
        selectors,
        data_attributes: all_data_attrs,
        url_patterns: all_url_patterns,
        pages_crawled: sorted_crawled,
        internal_urls,
    });

    if let Ok(raw_data) = &result {
        let _ = std::fs::create_dir_all(&cache_dir);
        if let Ok(json) = serde_json::to_string(raw_data) {
            let _ = std::fs::write(&cache_path, &json);
        }
    }

    result
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

    result.sort_by(|a, b| b.count.cmp(&a.count).then(a.selector.cmp(&b.selector)));
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

    let mut nav_links: Vec<String> = nav_links_set.into_iter().collect();
    nav_links.sort();
    all_links.sort();
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
    selected.push(main_url.to_string());

    let filtered: Vec<&String> = links
        .iter()
        .filter(|link| {
            let path = extract_path(link);
            path != "/"
                && !path.is_empty()
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
        .collect();

    let mut sorted_filtered: Vec<&String> = filtered;
    sorted_filtered.sort();

    let mut groups: std::collections::HashMap<String, Vec<&String>> = std::collections::HashMap::new();
    for link in &sorted_filtered {
        let first_seg = first_path_segment(link);
        groups.entry(first_seg).or_default().push(link);
    }

    let mut sorted_groups: Vec<(String, Vec<&String>)> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| a.0.cmp(&b.0));

    for (_group_key, group_links) in &mut sorted_groups {
        group_links.sort();
    }

    for (_group_key, group_links) in &sorted_groups {
        if selected.len() >= max {
            break;
        }
        let mut added = 0;
        for link in group_links {
            if selected.len() >= max {
                break;
            }
            let link_path = extract_path(link);
            if selected.iter().any(|s| extract_path(s) == link_path) {
                continue;
            }
            selected.push(link.to_string());
            added += 1;
            if added >= 2 {
                break;
            }
        }
    }

    selected
}

fn first_path_segment(url: &str) -> String {
    let path = extract_path(url);
    path.trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_string()
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
