use scraper::{ElementRef, Html, Node, Selector};

use super::scoring::{
    calculate_link_density, has_negative_pattern, score_element, should_remove_tag,
    LINK_DENSITY_THRESHOLD, MIN_PARAGRAPH_LENGTH, MIN_SCORE_THRESHOLD,
};

/// Find the main content root element in the document
pub fn find_content_root<'a>(document: &'a Html) -> Option<ElementRef<'a>> {
    // Try semantic elements first
    let semantic_selectors = ["article", "main", "[role='main']", "[role='article']"];

    for sel_str in semantic_selectors {
        if let Ok(selector) = Selector::parse(sel_str) {
            if let Some(element) = document.select(&selector).next() {
                // Verify it has substantial content
                let text_len = element.text().collect::<String>().trim().len();
                if text_len > 200 {
                    return Some(element);
                }
            }
        }
    }

    // Fallback to scoring algorithm
    find_by_scoring(document)
}

/// Find content root using scoring algorithm
fn find_by_scoring<'a>(document: &'a Html) -> Option<ElementRef<'a>> {
    let candidates_selector = Selector::parse("div, section, article, main, td").ok()?;

    let mut best_candidate: Option<ElementRef<'a>> = None;
    let mut best_score = MIN_SCORE_THRESHOLD;

    for element in document.select(&candidates_selector) {
        // Skip elements that should be removed
        if should_remove_tag(element.value().name()) {
            continue;
        }

        // Skip elements with negative patterns and low text
        if has_negative_pattern(&element) {
            let text_len = element.text().collect::<String>().trim().len();
            if text_len < 500 {
                continue;
            }
        }

        let score = score_element(&element);

        if score > best_score {
            best_score = score;
            best_candidate = Some(element);
        }
    }

    best_candidate
}

/// Collected content node for processing
#[derive(Debug, Clone)]
pub enum ContentNode {
    Element {
        tag: String,
        attrs: Vec<(String, String)>,
        children: Vec<ContentNode>,
    },
    Text(String),
}

impl ContentNode {
    pub fn is_empty(&self) -> bool {
        match self {
            ContentNode::Text(s) => s.trim().is_empty(),
            ContentNode::Element { children, .. } => {
                children.is_empty() || children.iter().all(|c| c.is_empty())
            }
        }
    }
}

/// Extract and clean content from the root element
pub fn extract_content(root: ElementRef) -> Vec<ContentNode> {
    collect_nodes(root)
}

fn collect_nodes(element: ElementRef) -> Vec<ContentNode> {
    let mut nodes = Vec::new();

    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                let s = text.text.to_string();
                // Normalize whitespace: collapse multiple spaces/newlines to single space
                // This matches HTML whitespace collapsing behavior
                let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");

                // Keep text nodes even if whitespace-only (important for inline spacing)
                if normalized.is_empty() {
                    // If original contained whitespace, preserve as single space
                    if s.chars().any(|c| c.is_whitespace()) {
                        nodes.push(ContentNode::Text(" ".to_string()));
                    }
                } else {
                    // Preserve leading/trailing space if original had it
                    let has_leading = s.starts_with(char::is_whitespace);
                    let has_trailing = s.ends_with(char::is_whitespace);

                    let mut result = String::new();
                    if has_leading {
                        result.push(' ');
                    }
                    result.push_str(&normalized);
                    if has_trailing {
                        result.push(' ');
                    }
                    nodes.push(ContentNode::Text(result));
                }
            }
            Node::Element(el) => {
                let tag_name = el.name();

                // Skip tags to remove
                if should_remove_tag(tag_name) {
                    continue;
                }

                let child_ref = ElementRef::wrap(child).unwrap();

                // Skip elements with negative patterns and high link density
                if has_negative_pattern(&child_ref) {
                    let link_density = calculate_link_density(&child_ref);
                    if link_density > LINK_DENSITY_THRESHOLD {
                        continue;
                    }
                }

                // Skip short paragraphs without images
                if tag_name == "p" {
                    let text_len = child_ref.text().collect::<String>().trim().len();
                    let has_img = Selector::parse("img")
                        .ok()
                        .map(|s| child_ref.select(&s).next().is_some())
                        .unwrap_or(false);

                    if text_len < MIN_PARAGRAPH_LENGTH && !has_img {
                        continue;
                    }
                }

                // Collect attributes we care about
                let attrs: Vec<(String, String)> = el
                    .attrs()
                    .filter(|(name, _)| {
                        matches!(
                            *name,
                            "href" | "src" | "alt" | "class" | "id" | "lang" | "title"
                        )
                    })
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                let children = collect_nodes(child_ref);

                // Skip empty container elements
                if is_container_tag(tag_name) && children.iter().all(|c| c.is_empty()) {
                    continue;
                }

                nodes.push(ContentNode::Element {
                    tag: tag_name.to_string(),
                    attrs,
                    children,
                });
            }
            _ => {}
        }
    }

    nodes
}

fn is_container_tag(tag: &str) -> bool {
    matches!(
        tag,
        "div" | "span" | "section" | "article" | "main" | "figure" | "figcaption"
    )
}

/// Extract metadata from document
pub fn extract_metadata(document: &Html) -> (Option<String>, Option<String>) {
    let title = extract_title(document);
    let description = extract_description(document);
    (title, description)
}

fn extract_title(document: &Html) -> Option<String> {
    // 1. og:title
    if let Some(title) = get_meta_content(document, "meta[property='og:title']") {
        return Some(title);
    }

    // 2. twitter:title
    if let Some(title) = get_meta_content(document, "meta[name='twitter:title']") {
        return Some(title);
    }

    // 3. <title> tag (cleaned)
    if let Ok(selector) = Selector::parse("title") {
        if let Some(element) = document.select(&selector).next() {
            let title = element.text().collect::<String>();
            return Some(clean_title(&title));
        }
    }

    // 4. First h1
    if let Ok(selector) = Selector::parse("h1") {
        if let Some(element) = document.select(&selector).next() {
            return Some(element.text().collect::<String>().trim().to_string());
        }
    }

    None
}

fn extract_description(document: &Html) -> Option<String> {
    // 1. og:description
    if let Some(desc) = get_meta_content(document, "meta[property='og:description']") {
        return Some(desc);
    }

    // 2. meta description
    if let Some(desc) = get_meta_content(document, "meta[name='description']") {
        return Some(desc);
    }

    // 3. First paragraph (will be set during conversion if not found)
    None
}

fn get_meta_content(document: &Html, selector_str: &str) -> Option<String> {
    let selector = Selector::parse(selector_str).ok()?;
    let element = document.select(&selector).next()?;
    element
        .value()
        .attr("content")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Clean title by removing site name suffix
fn clean_title(title: &str) -> String {
    let title = title.trim();

    // Common separators: " | ", " - ", " — ", " :: ", " » "
    let separators = [" | ", " - ", " — ", " :: ", " » ", " : "];

    for sep in separators {
        if let Some(pos) = title.rfind(sep) {
            // Keep the longer part
            let left = &title[..pos];
            let right = &title[pos + sep.len()..];

            if left.len() > right.len() {
                return left.trim().to_string();
            }
        }
    }

    title.to_string()
}
