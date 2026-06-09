use async_trait::async_trait;
use drova_sdk::requester::{Error, ProtocolHandler, Response, ResponseData};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

const DEFAULT_PORT: u16 = 70;

/// Gopher item types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemType {
    /// Text file
    Text,
    /// Directory listing (gophermap)
    Directory,
    /// CSO phone-book server
    CsoPhoneBook,
    /// Error
    Error,
    /// BinHex encoded file
    BinHex,
    /// DOS binary archive
    DosBinary,
    /// Unix uuencoded file
    UuEncoded,
    /// Index-Search server
    Search,
    /// Telnet session
    Telnet,
    /// Binary file
    Binary,
    /// Redundant server
    Redundant,
    /// TN3270 session
    Tn3270,
    /// GIF image
    Gif,
    /// Image file
    Image,
    /// HTML file
    Html,
    /// Informational message
    Info,
    /// Sound file
    Sound,
    /// Unknown type
    Unknown(char),
}

impl ItemType {
    /// Parse item type from character
    pub fn from_char(c: char) -> Self {
        match c {
            '0' => ItemType::Text,
            '1' => ItemType::Directory,
            '2' => ItemType::CsoPhoneBook,
            '3' => ItemType::Error,
            '4' => ItemType::BinHex,
            '5' => ItemType::DosBinary,
            '6' => ItemType::UuEncoded,
            '7' => ItemType::Search,
            '8' => ItemType::Telnet,
            '9' => ItemType::Binary,
            '+' => ItemType::Redundant,
            'T' => ItemType::Tn3270,
            'g' => ItemType::Gif,
            'I' => ItemType::Image,
            'h' => ItemType::Html,
            'i' => ItemType::Info,
            's' => ItemType::Sound,
            c => ItemType::Unknown(c),
        }
    }

    /// Convert item type to character
    pub fn to_char(&self) -> char {
        match self {
            ItemType::Text => '0',
            ItemType::Directory => '1',
            ItemType::CsoPhoneBook => '2',
            ItemType::Error => '3',
            ItemType::BinHex => '4',
            ItemType::DosBinary => '5',
            ItemType::UuEncoded => '6',
            ItemType::Search => '7',
            ItemType::Telnet => '8',
            ItemType::Binary => '9',
            ItemType::Redundant => '+',
            ItemType::Tn3270 => 'T',
            ItemType::Gif => 'g',
            ItemType::Image => 'I',
            ItemType::Html => 'h',
            ItemType::Info => 'i',
            ItemType::Sound => 's',
            ItemType::Unknown(c) => *c,
        }
    }

    /// Get MIME type for this item type
    pub fn mime_type(&self) -> &'static str {
        match self {
            ItemType::Text => "text/plain",
            ItemType::Directory => "text/x-gophermap",
            ItemType::Html => "text/html",
            ItemType::Gif => "image/gif",
            ItemType::Image => "image/png",
            ItemType::Sound => "audio/basic",
            ItemType::Binary | ItemType::DosBinary | ItemType::BinHex | ItemType::UuEncoded => {
                "application/octet-stream"
            }
            ItemType::Error => "text/plain",
            ItemType::Info => "text/plain",
            _ => "application/octet-stream",
        }
    }

    /// Check if this type returns text content
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            ItemType::Text | ItemType::Directory | ItemType::Html | ItemType::Error | ItemType::Info
        )
    }
}

pub struct GopherProtocol;

#[async_trait]
impl ProtocolHandler for GopherProtocol {
    async fn fetch(&self, url: &Url) -> Result<Response, Error> {
        let host = url.host_str().ok_or(Error::InvalidUrl)?;
        let port = url.port().unwrap_or(DEFAULT_PORT);

        // Parse gopher URL path
        // Format: /type/selector or /selector (type defaults to 1)
        let path = url.path();
        let (item_type, selector) = parse_gopher_path(path);

        // Handle search queries (type 7)
        let selector = if item_type == ItemType::Search {
            if let Some(query) = url.query() {
                format!("{}\t{}", selector, query)
            } else {
                selector.to_string()
            }
        } else {
            selector.to_string()
        };

        // Connect to server
        let addr = format!("{}:{}", host, port);
        let mut stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| Error::IoError(e.to_string()))?;

        // Send selector
        let request = format!("{}\r\n", selector);
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| Error::IoError(e.to_string()))?;

        // Read response
        let mut buffer = Vec::new();
        stream
            .read_to_end(&mut buffer)
            .await
            .map_err(|e| Error::IoError(e.to_string()))?;

        let mime_type = item_type.mime_type().to_string();

        if item_type.is_text() {
            // Convert to string, handling encoding
            let text = String::from_utf8(buffer)
                .or_else(|e| {
                    // Try lossy conversion for non-UTF8 content
                    Ok::<String, Error>(String::from_utf8_lossy(e.as_bytes()).into_owned())
                })
                .map_err(|_| Error::InvalidEncoding)?;

            // Remove trailing period if present (Gopher end-of-text marker)
            let text = text
                .strip_suffix(".\r\n")
                .or_else(|| text.strip_suffix(".\n"))
                .unwrap_or(&text)
                .to_string();

            Ok(Response {
                data: ResponseData::TextOutput(text),
                ty: mime_type,
            })
        } else {
            Ok(Response {
                data: ResponseData::BitsOutput(buffer),
                ty: mime_type,
            })
        }
    }
}

/// Parse gopher URL path into item type and selector
/// Path format: /type/selector or /selector (type defaults to '1')
fn parse_gopher_path(path: &str) -> (ItemType, &str) {
    let path = path.strip_prefix('/').unwrap_or(path);

    if path.is_empty() {
        return (ItemType::Directory, "");
    }

    let mut chars = path.chars();
    let first_char = chars.next().unwrap();

    // Check if first character is a valid type indicator
    // followed by nothing or a '/'
    let rest = chars.as_str();

    if rest.is_empty() {
        // Single character path - could be type or selector
        if is_likely_type_char(first_char) {
            return (ItemType::from_char(first_char), "");
        } else {
            return (ItemType::Directory, path);
        }
    }

    if rest.starts_with('/') {
        // Format: /type/selector
        let selector = rest.strip_prefix('/').unwrap_or(rest);
        return (ItemType::from_char(first_char), selector);
    }

    // No type indicator, treat whole path as selector
    (ItemType::Directory, path)
}

/// Check if character is likely a gopher type indicator
fn is_likely_type_char(c: char) -> bool {
    matches!(
        c,
        '0' | '1'
            | '2'
            | '3'
            | '4'
            | '5'
            | '6'
            | '7'
            | '8'
            | '9'
            | '+'
            | 'T'
            | 'g'
            | 'I'
            | 'h'
            | 'i'
            | 's'
    )
}
