use scraper::{ElementRef, Selector};

// === READABILITY CONFIG ===
// Modify these constants to tune the algorithm

/// Minimum text length for an element to be considered content
pub const MIN_CONTENT_LENGTH: usize = 140;

/// Minimum score threshold for content candidates
pub const MIN_SCORE_THRESHOLD: i32 = 20;

/// Maximum link density before element is considered navigation
pub const LINK_DENSITY_THRESHOLD: f32 = 0.33;

/// Minimum paragraph length to keep during cleaning
pub const MIN_PARAGRAPH_LENGTH: usize = 25;

/// Patterns in class/id that indicate main content
pub const POSITIVE_PATTERNS: &[&str] = &[
    "article",
    "content",
    "post",
    "entry",
    "text",
    "body",
    "main",
    "story",
    "blog",
    "page",
    "hentry",
    "entry-content",
    "post-content",
    "article-content",
];

/// Patterns in class/id that indicate non-content
pub const NEGATIVE_PATTERNS: &[&str] = &[
    "sidebar",
    "side-bar",
    "side_bar",
    "nav",
    "menu",
    "navigation",
    "ad",
    "ads",
    "advert",
    "advertisement",
    "banner",
    "footer",
    "header",
    "comment",
    "comments",
    "respond",
    "widget",
    "social",
    "share",
    "related",
    "recommend",
    "popup",
    "modal",
    "cookie",
    "newsletter",
    "subscribe",
    "promo",
    "sponsor",
    "masthead",
    "breadcrumb",
    "pagination",
    "pager",
    "meta",
    "author-info",
    "byline",
    "tag-list",
    "category",
];

/// Tags to completely remove from DOM
pub const TAGS_TO_REMOVE: &[&str] = &[
    "script", "style", "noscript", "iframe", "form", "button", "input", "select", "textarea",
    "svg", "canvas", "aside", "nav", "footer", "header",
];

/// Base scores for different HTML tags
pub const TAG_BASE_SCORES: &[(&str, i32)] = &[
    ("article", 30),
    ("main", 25),
    ("section", 15),
    ("div", 5),
    ("p", 3),
    ("pre", 3),
    ("td", 3),
    ("blockquote", 3),
    ("figure", 5),
];

/// Weight multiplier for positive pattern matches
pub const POSITIVE_WEIGHT: i32 = 25;

/// Weight multiplier for negative pattern matches
pub const NEGATIVE_WEIGHT: i32 = 25;

/// Bonus points per paragraph found inside element
pub const PARAGRAPH_BONUS: i32 = 3;

/// Penalty for high link density
pub const LINK_DENSITY_PENALTY: i32 = 30;

/// Bonus points per 100 characters of text
pub const TEXT_LENGTH_BONUS_PER_100: i32 = 1;

/// Calculate score for an element
pub fn score_element(element: &ElementRef) -> i32 {
    let mut score = 0;

    let tag_name = element.value().name();

    // Base tag score
    score += TAG_BASE_SCORES
        .iter()
        .find(|(tag, _)| *tag == tag_name)
        .map(|(_, s)| *s)
        .unwrap_or(0);

    // Class/ID scoring
    let class_id = format!(
        "{} {}",
        element.value().attr("class").unwrap_or(""),
        element.value().attr("id").unwrap_or("")
    )
    .to_lowercase();

    for pattern in POSITIVE_PATTERNS {
        if class_id.contains(pattern) {
            score += POSITIVE_WEIGHT;
        }
    }

    for pattern in NEGATIVE_PATTERNS {
        if class_id.contains(pattern) {
            score -= NEGATIVE_WEIGHT;
        }
    }

    // Text density bonus
    let text_len = element.text().collect::<String>().trim().len();
    if text_len > MIN_CONTENT_LENGTH {
        score += (text_len / 100) as i32 * TEXT_LENGTH_BONUS_PER_100;
    }

    // Link density penalty
    let link_density = calculate_link_density(element);
    if link_density > LINK_DENSITY_THRESHOLD {
        score -= LINK_DENSITY_PENALTY;
    }

    // Paragraph count bonus
    if let Ok(selector) = Selector::parse("p") {
        let p_count = element.select(&selector).count();
        score += (p_count as i32) * PARAGRAPH_BONUS;
    }

    score
}

/// Calculate the ratio of link text to total text
pub fn calculate_link_density(element: &ElementRef) -> f32 {
    let text_len = element.text().collect::<String>().len() as f32;
    if text_len == 0.0 {
        return 1.0;
    }

    let link_text_len: usize = Selector::parse("a")
        .ok()
        .map(|selector| {
            element
                .select(&selector)
                .map(|a| a.text().collect::<String>().len())
                .sum()
        })
        .unwrap_or(0);

    link_text_len as f32 / text_len
}

/// Check if element should be removed based on class/id patterns
pub fn has_negative_pattern(element: &ElementRef) -> bool {
    let class_id = format!(
        "{} {}",
        element.value().attr("class").unwrap_or(""),
        element.value().attr("id").unwrap_or("")
    )
    .to_lowercase();

    NEGATIVE_PATTERNS.iter().any(|p| class_id.contains(p))
}

/// Check if element's tag should be completely removed
pub fn should_remove_tag(tag_name: &str) -> bool {
    TAGS_TO_REMOVE.contains(&tag_name)
}
