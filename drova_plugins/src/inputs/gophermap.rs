use dalet::types::{
    Page,
    Tag::{self, *},
};
use drova_sdk::requester::{Error, InputHandler};
use url::Url;

use crate::protocols::GopherItemType as ItemType;

pub struct GophermapInput;

impl InputHandler for GophermapInput {
    fn process_text(&self, s: String, url: Option<&Url>) -> Result<Page, Error> {
        let mut page: Vec<Tag> = Vec::new();
        let title: Option<String> = None;
        let mut description: Option<String> = None;

        // Track consecutive info lines for grouping
        let mut info_buffer: Vec<String> = Vec::new();

        for line in s.lines() {
            // Skip empty lines and end marker
            if line.is_empty() || line == "." {
                continue;
            }

            let entry = parse_gophermap_line(line);

            match entry.item_type {
                ItemType::Info => {
                    // Collect info lines
                    info_buffer.push(entry.display.to_string());
                }
                _ => {
                    // Flush info buffer as paragraph
                    if !info_buffer.is_empty() {
                        let text = info_buffer.join("\n");
                        if description.is_none() && text.len() > 10 {
                            description = Some(truncate(&text, 150).to_string());
                        }
                        page.push(Paragraph { body: text.into() });
                        info_buffer.clear();
                    }

                    // Process the entry
                    if let Some(tag) = convert_entry(&entry, url) {
                        page.push(tag);
                    }
                }
            }
        }

        // Flush remaining info lines
        if !info_buffer.is_empty() {
            let text = info_buffer.join("\n");
            if description.is_none() && text.len() > 10 {
                description = Some(truncate(&text, 150).to_string());
            }
            page.push(Paragraph { body: text.into() });
        }

        Ok(Page {
            title,
            description,
            body: page,
            variables: None,
        })
    }

    fn process_bytes(&self, _: Vec<u8>, _: Option<&Url>) -> Result<Page, Error> {
        Err(Error::UnsupportedInput)
    }
}

/// Parsed gophermap entry
struct GophermapEntry<'a> {
    item_type: ItemType,
    display: &'a str,
    selector: &'a str,
    host: &'a str,
    port: u16,
}

/// Parse a single gophermap line
/// Format: TypeDisplay<TAB>Selector<TAB>Host<TAB>Port
fn parse_gophermap_line(line: &str) -> GophermapEntry<'_> {
    let mut chars = line.chars();
    let type_char = chars.next().unwrap_or('i');
    let rest = chars.as_str();

    let parts: Vec<&str> = rest.split('\t').collect();

    let display = parts.first().copied().unwrap_or("");
    let selector = parts.get(1).copied().unwrap_or("");
    let host = parts.get(2).copied().unwrap_or("");
    let port = parts
        .get(3)
        .and_then(|p| p.parse().ok())
        .unwrap_or(70);

    GophermapEntry {
        item_type: ItemType::from_char(type_char),
        display,
        selector,
        host,
        port,
    }
}

/// Convert gophermap entry to Dalet tag
fn convert_entry(entry: &GophermapEntry, base_url: Option<&Url>) -> Option<Tag> {
    match entry.item_type {
        ItemType::Info => {
            // Info items are just text, no link
            if entry.display.is_empty() {
                None
            } else {
                Some(Paragraph {
                    body: entry.display.into(),
                })
            }
        }

        ItemType::Error => {
            // Error message
            Some(BlockQuote {
                body: format!("Error: {}", entry.display).into(),
            })
        }

        ItemType::Directory => {
            // Directory link
            let url = build_gopher_url(entry, base_url);
            Some(Paragraph {
                body: vec![NavLink {
                    body: Some(format!("[DIR] {}", entry.display).into()),
                    dref: url,
                }]
                .into(),
            })
        }

        ItemType::Text => {
            // Text file link
            let url = build_gopher_url(entry, base_url);
            Some(Paragraph {
                body: vec![NavLink {
                    body: Some(format!("[TXT] {}", entry.display).into()),
                    dref: url,
                }]
                .into(),
            })
        }

        ItemType::Html => {
            // HTML link - selector often contains URL: prefix
            let url = if entry.selector.starts_with("URL:") {
                entry.selector.strip_prefix("URL:").unwrap().to_string()
            } else {
                build_gopher_url(entry, base_url)
            };
            Some(Paragraph {
                body: vec![NavLink {
                    body: Some(format!("[WWW] {}", entry.display).into()),
                    dref: url,
                }]
                .into(),
            })
        }

        ItemType::Search => {
            // Search server
            let url = build_gopher_url(entry, base_url);
            Some(Paragraph {
                body: vec![NavLink {
                    body: Some(format!("[SEARCH] {}", entry.display).into()),
                    dref: url,
                }]
                .into(),
            })
        }

        ItemType::Binary | ItemType::DosBinary => {
            // Binary file
            let url = build_gopher_url(entry, base_url);
            Some(Paragraph {
                body: vec![NavLink {
                    body: Some(format!("[BIN] {}", entry.display).into()),
                    dref: url,
                }]
                .into(),
            })
        }

        ItemType::Gif | ItemType::Image => {
            // Image - could display inline or as link
            let url = build_gopher_url(entry, base_url);
            Some(Image {
                src: url,
                alt: Some(entry.display.to_string()),
            })
        }

        ItemType::Sound => {
            // Sound file
            let url = build_gopher_url(entry, base_url);
            Some(Paragraph {
                body: vec![NavLink {
                    body: Some(format!("[AUDIO] {}", entry.display).into()),
                    dref: url,
                }]
                .into(),
            })
        }

        ItemType::Telnet | ItemType::Tn3270 => {
            // Telnet session - just show as text since we can't handle it
            Some(Paragraph {
                body: format!("[TELNET] {} ({}:{})", entry.display, entry.host, entry.port).into(),
            })
        }

        _ => {
            // Unknown type - create generic link
            if !entry.selector.is_empty() && !entry.host.is_empty() {
                let url = build_gopher_url(entry, base_url);
                Some(Paragraph {
                    body: vec![NavLink {
                        body: Some(entry.display.into()),
                        dref: url,
                    }]
                    .into(),
                })
            } else if !entry.display.is_empty() {
                Some(Paragraph {
                    body: entry.display.into(),
                })
            } else {
                None
            }
        }
    }
}

/// Build gopher URL from entry
fn build_gopher_url(entry: &GophermapEntry, base_url: Option<&Url>) -> String {
    let type_char = entry.item_type.to_char();

    // Use entry's host/port, or fall back to base URL
    let (host, port) = if !entry.host.is_empty() {
        (entry.host.to_string(), entry.port)
    } else if let Some(base) = base_url {
        (
            base.host_str().unwrap_or("").to_string(),
            base.port().unwrap_or(70),
        )
    } else {
        return entry.selector.to_string();
    };

    if port == 70 {
        format!("gopher://{}/{}/{}", host, type_char, entry.selector)
    } else {
        format!("gopher://{}:{}/{}/{}", host, port, type_char, entry.selector)
    }
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
