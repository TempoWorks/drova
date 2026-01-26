use dalet::types::{HeadingLevel, ListStyle, Page, TableRows, Tag, Text};
use url::Url;

use super::readability::ContentNode;

/// Convert content nodes to Dalet page body
pub fn convert_to_dalet(
    nodes: &[ContentNode],
    base_url: Option<&Url>,
    page: &mut Page,
) -> Vec<Tag> {
    let mut tags = Vec::new();

    for node in nodes {
        if let Some(tag) = convert_node(node, base_url, page) {
            tags.push(tag);
        }
    }

    tags
}

fn convert_node(node: &ContentNode, base_url: Option<&Url>, page: &mut Page) -> Option<Tag> {
    match node {
        ContentNode::Text(s) => {
            // Whitespace is already normalized in readability.rs
            // Just skip completely empty strings
            if s.is_empty() {
                None
            } else {
                Some(Tag::Element {
                    body: s.clone().into(),
                })
            }
        }
        ContentNode::Element {
            tag,
            attrs,
            children,
        } => convert_element(tag, attrs, children, base_url, page),
    }
}

fn convert_element(
    tag: &str,
    attrs: &[(String, String)],
    children: &[ContentNode],
    base_url: Option<&Url>,
    page: &mut Page,
) -> Option<Tag> {
    match tag {
        // Headings
        "h1" => {
            let text = nodes_to_text(children);
            if page.title.is_none() && !text.is_empty() {
                page.title = Some(text.clone());
            }
            Some(Tag::Heading {
                body: text,
                heading: HeadingLevel::One,
            })
        }
        "h2" => Some(Tag::Heading {
            body: nodes_to_text(children),
            heading: HeadingLevel::Two,
        }),
        "h3" => Some(Tag::Heading {
            body: nodes_to_text(children),
            heading: HeadingLevel::Three,
        }),
        "h4" => Some(Tag::Heading {
            body: nodes_to_text(children),
            heading: HeadingLevel::Four,
        }),
        "h5" => Some(Tag::Heading {
            body: nodes_to_text(children),
            heading: HeadingLevel::Five,
        }),
        "h6" => Some(Tag::Heading {
            body: nodes_to_text(children),
            heading: HeadingLevel::Six,
        }),

        // Paragraph
        "p" => {
            let body = convert_to_dalet(children, base_url, page);
            if body.is_empty() {
                return None;
            }

            // Set description from first paragraph
            if page.description.is_none() {
                page.description = Some(nodes_to_text(children));
            }

            Some(Tag::Paragraph { body: body.into() })
        }

        // Block elements
        "blockquote" => {
            let body = convert_to_dalet(children, base_url, page);
            if body.is_empty() {
                None
            } else {
                Some(Tag::BlockQuote { body: body.into() })
            }
        }

        "pre" => {
            // Check for code inside
            if let Some(code_node) = find_child_by_tag(children, "code") {
                if let ContentNode::Element {
                    attrs, children, ..
                } = code_node
                {
                    let language = extract_language(attrs);
                    let text = nodes_to_text(children);
                    return Some(Tag::Code {
                        body: text,
                        language,
                    });
                }
            }
            Some(Tag::Code {
                body: nodes_to_text(children),
                language: None,
            })
        }

        "code" => {
            let language = extract_language(attrs);
            Some(Tag::InlineCode {
                body: nodes_to_text(children),
                language,
            })
        }

        // Formatting
        "strong" | "b" => Some(Tag::Bold {
            body: nodes_to_text(children),
        }),

        "em" | "i" => Some(Tag::Italic {
            body: nodes_to_text(children),
        }),

        "del" | "s" | "strike" => Some(Tag::Strikethrough {
            body: nodes_to_text(children),
        }),

        "sup" => Some(Tag::Superscript {
            body: nodes_to_text(children),
        }),

        "sub" => Some(Tag::Subscript {
            body: nodes_to_text(children),
        }),

        "mark" | "u" => {
            // No direct mapping, use bold as fallback
            Some(Tag::Bold {
                body: nodes_to_text(children),
            })
        }

        // Links
        "a" => {
            let href = get_attr(attrs, "href").unwrap_or_default();
            if href.is_empty() || href.starts_with("javascript:") {
                // Just return children as inline
                let body = convert_to_dalet(children, base_url, page);
                return if body.is_empty() {
                    None
                } else {
                    Some(Tag::Element { body: body.into() })
                };
            }

            let resolved = resolve_url(&href, base_url);
            let body = convert_to_dalet(children, base_url, page);

            Some(Tag::Link {
                dref: resolved,
                body: if body.is_empty() {
                    None
                } else {
                    Some(body.into())
                },
            })
        }

        // Images
        "img" => {
            let src = get_attr(attrs, "src")?;
            if src.is_empty() {
                return None;
            }

            let resolved = resolve_url(&src, base_url);
            let alt = get_attr(attrs, "alt");

            Some(Tag::Image {
                src: resolved,
                alt,
            })
        }

        // === FUTURE MEDIA STUBS ===
        // TODO: Uncomment when Dalet supports these types
        //
        // "video" => {
        //     let src = get_attr(attrs, "src")
        //         .or_else(|| find_source_src(children));
        //     if let Some(src) = src {
        //         let resolved = resolve_url(&src, base_url);
        //         // return Some(Tag::Video { src: resolved, ... });
        //     }
        //     None
        // }
        //
        // "audio" => {
        //     let src = get_attr(attrs, "src")
        //         .or_else(|| find_source_src(children));
        //     if let Some(src) = src {
        //         let resolved = resolve_url(&src, base_url);
        //         // return Some(Tag::Audio { src: resolved, ... });
        //     }
        //     None
        // }
        //
        // "iframe" => {
        //     let src = get_attr(attrs, "src")?;
        //     if is_video_embed(&src) {
        //         let resolved = resolve_url(&src, base_url);
        //         // return Some(Tag::Embed { src: resolved, ... });
        //     }
        //     None
        // }
        "video" | "audio" | "iframe" => {
            // Currently unsupported, skip
            None
        }

        // Lists
        "ul" => {
            let items = convert_list_items(children, base_url, page);
            if items.is_empty() {
                None
            } else {
                Some(Tag::List {
                    body: items,
                    style: ListStyle::Disc,
                })
            }
        }

        "ol" => {
            let items = convert_list_items(children, base_url, page);
            if items.is_empty() {
                None
            } else {
                Some(Tag::List {
                    body: items,
                    style: ListStyle::Decimal,
                })
            }
        }

        "li" => {
            let body = convert_to_dalet(children, base_url, page);
            if body.is_empty() {
                None
            } else {
                Some(Tag::Element { body: body.into() })
            }
        }

        // Tables
        "table" => {
            let rows = convert_table(children, base_url, page);
            if rows.is_empty() {
                None
            } else {
                Some(Tag::Table { body: rows })
            }
        }

        // Horizontal rule
        "hr" => Some(Tag::HorizontalBreak),

        // Line break
        "br" => Some(Tag::Element {
            body: "\n".to_string().into(),
        }),

        // Figure with caption
        "figure" => {
            let body = convert_to_dalet(children, base_url, page);
            if body.is_empty() {
                None
            } else {
                Some(Tag::Block { body })
            }
        }

        "figcaption" => {
            let text = nodes_to_text(children);
            if text.is_empty() {
                None
            } else {
                Some(Tag::Paragraph {
                    body: text.into(),
                })
            }
        }

        // Container elements - transparent, just process children
        "div" | "span" | "section" | "article" | "main" | "aside" | "header" | "footer"
        | "address" | "details" | "summary" | "time" | "abbr" | "cite" | "q" | "dfn" | "kbd"
        | "samp" | "var" | "small" | "data" | "picture" | "source" => {
            let body = convert_to_dalet(children, base_url, page);
            if body.is_empty() {
                None
            } else if body.len() == 1 {
                // Unwrap single child
                Some(body.into_iter().next().unwrap())
            } else {
                Some(Tag::Element { body: body.into() })
            }
        }

        // Unknown tags - try to extract content
        _ => {
            let body = convert_to_dalet(children, base_url, page);
            if body.is_empty() {
                None
            } else if body.len() == 1 {
                Some(body.into_iter().next().unwrap())
            } else {
                Some(Tag::Element { body: body.into() })
            }
        }
    }
}

fn convert_list_items(
    children: &[ContentNode],
    base_url: Option<&Url>,
    page: &mut Page,
) -> Vec<Tag> {
    children
        .iter()
        .filter_map(|node| {
            if let ContentNode::Element { tag, children, .. } = node {
                if tag == "li" {
                    let body = convert_to_dalet(children, base_url, page);
                    if body.is_empty() {
                        return None;
                    }
                    return Some(Tag::Element { body: body.into() });
                }
            }
            None
        })
        .collect()
}

fn convert_table(
    children: &[ContentNode],
    base_url: Option<&Url>,
    page: &mut Page,
) -> Vec<TableRows> {
    let mut rows = Vec::new();
    collect_table_rows(children, base_url, page, &mut rows);
    rows
}

fn collect_table_rows(
    children: &[ContentNode],
    base_url: Option<&Url>,
    page: &mut Page,
    rows: &mut Vec<TableRows>,
) {
    for node in children {
        if let ContentNode::Element { tag, children, .. } = node {
            match tag.as_str() {
                "thead" | "tbody" | "tfoot" => {
                    collect_table_rows(children, base_url, page, rows);
                }
                "tr" => {
                    let (is_header, cells) = convert_table_row(children, base_url, page);
                    if !cells.is_empty() {
                        if is_header {
                            rows.push(TableRows::Primary(cells));
                        } else {
                            rows.push(TableRows::Default(cells));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn convert_table_row(
    children: &[ContentNode],
    base_url: Option<&Url>,
    page: &mut Page,
) -> (bool, Vec<Tag>) {
    let mut cells = Vec::new();
    let mut is_header = false;

    for node in children {
        if let ContentNode::Element { tag, children, .. } = node {
            match tag.as_str() {
                "th" => {
                    is_header = true;
                    let body = convert_to_dalet(children, base_url, page);
                    cells.push(Tag::Element { body: body.into() });
                }
                "td" => {
                    let body = convert_to_dalet(children, base_url, page);
                    cells.push(Tag::Element { body: body.into() });
                }
                _ => {}
            }
        }
    }

    (is_header, cells)
}

/// Extract text content from nodes
pub fn nodes_to_text(nodes: &[ContentNode]) -> Text {
    let mut output = String::new();

    for node in nodes {
        match node {
            ContentNode::Text(s) => output.push_str(s),
            ContentNode::Element { tag, children, .. } => {
                // Add space before block elements
                if is_block_element(tag) && !output.is_empty() && !output.ends_with(' ') {
                    output.push(' ');
                }

                output.push_str(&nodes_to_text(children));

                // Add space after block elements
                if is_block_element(tag) && !output.ends_with(' ') {
                    output.push(' ');
                }
            }
        }
    }

    output.trim().to_string()
}

fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "pre"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "tr"
            | "br"
            | "hr"
    )
}

fn get_attr(attrs: &[(String, String)], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
}

fn find_child_by_tag<'a>(children: &'a [ContentNode], tag_name: &str) -> Option<&'a ContentNode> {
    children.iter().find(|node| {
        if let ContentNode::Element { tag, .. } = node {
            tag == tag_name
        } else {
            false
        }
    })
}

fn extract_language(attrs: &[(String, String)]) -> Option<String> {
    let class = get_attr(attrs, "class")?;

    // Look for language-* or lang-* pattern
    for part in class.split_whitespace() {
        if let Some(lang) = part.strip_prefix("language-") {
            return Some(lang.to_string());
        }
        if let Some(lang) = part.strip_prefix("lang-") {
            return Some(lang.to_string());
        }
    }

    None
}

fn resolve_url(href: &str, base: Option<&Url>) -> String {
    // Data URLs and absolute URLs
    if href.starts_with("data:")
        || href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("//")
    {
        if href.starts_with("//") {
            return format!("https:{}", href);
        }
        return href.to_string();
    }

    match base {
        Some(base_url) => base_url
            .join(href)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| href.to_string()),
        None => href.to_string(),
    }
}

// === FUTURE MEDIA HELPERS ===
// TODO: Uncomment when needed
//
// fn find_source_src(children: &[ContentNode]) -> Option<String> {
//     for node in children {
//         if let ContentNode::Element { tag, attrs, .. } = node {
//             if tag == "source" {
//                 if let Some(src) = get_attr(attrs, "src") {
//                     return Some(src);
//                 }
//             }
//         }
//     }
//     None
// }
//
// fn is_video_embed(src: &str) -> bool {
//     src.contains("youtube.com")
//         || src.contains("youtu.be")
//         || src.contains("vimeo.com")
//         || src.contains("dailymotion.com")
// }
