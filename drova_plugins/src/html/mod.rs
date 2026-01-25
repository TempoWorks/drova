use dalet::types::Page;
use drova_sdk::requester::{Error, InputHandler};
use scraper::Html;
use url::Url;

mod convert;
mod readability;
mod scoring;

pub use scoring::{
    LINK_DENSITY_THRESHOLD, MIN_CONTENT_LENGTH, MIN_PARAGRAPH_LENGTH, MIN_SCORE_THRESHOLD,
    NEGATIVE_PATTERNS, POSITIVE_PATTERNS, TAGS_TO_REMOVE, TAG_BASE_SCORES,
};

/// HTML input handler with readability-style content extraction
pub struct HtmlInput;

impl InputHandler for HtmlInput {
    fn process_text(&self, html: String, url: Option<&Url>) -> Result<Page, Error> {
        // Parse HTML document
        let document = Html::parse_document(&html);

        // Extract metadata
        let (title, description) = readability::extract_metadata(&document);

        // Find main content root
        let content_root = readability::find_content_root(&document)
            .ok_or(Error::ParserError("No content found".to_string()))?;

        // Extract and clean content
        let content_nodes = readability::extract_content(content_root);

        // Convert to Dalet
        let mut page = Page {
            title,
            description,
            body: vec![],
            variables: None,
        };

        let body = convert::convert_to_dalet(&content_nodes, url, &mut page);
        page.body = body;

        // Ensure we have some content
        if page.body.is_empty() {
            return Err(Error::ParserError("No content extracted".to_string()));
        }

        Ok(page)
    }

    fn process_bytes(&self, bytes: Vec<u8>, url: Option<&Url>) -> Result<Page, Error> {
        let html =
            readability::decode_html_bytes(&bytes).map_err(|e| Error::ParserError(e.to_string()))?;

        self.process_text(html, url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_article() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Test Article | My Site</title>
                <meta property="og:description" content="This is a test article">
            </head>
            <body>
                <nav>Navigation links here</nav>
                <article>
                    <h1>Test Article</h1>
                    <p>This is the first paragraph with enough content to be considered valid content for extraction.</p>
                    <p>This is another paragraph with some <strong>bold</strong> and <em>italic</em> text.</p>
                </article>
                <footer>Footer content</footer>
            </body>
            </html>
        "#;

        let input = HtmlInput;
        let result = input.process_text(html.to_string(), None);

        assert!(result.is_ok());
        let page = result.unwrap();

        assert_eq!(page.title, Some("Test Article".to_string()));
        assert_eq!(
            page.description,
            Some("This is a test article".to_string())
        );
        assert!(!page.body.is_empty());
    }

    #[test]
    fn test_no_article_fallback() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Blog Post</title></head>
            <body>
                <div class="content">
                    <h1>Blog Post Title</h1>
                    <p>This is a blog post with substantial content that should be detected by the scoring algorithm even without semantic article tags.</p>
                    <p>Another paragraph here to add more weight to the content scoring.</p>
                </div>
                <div class="sidebar">
                    <a href="/link1">Link 1</a>
                    <a href="/link2">Link 2</a>
                </div>
            </body>
            </html>
        "#;

        let input = HtmlInput;
        let result = input.process_text(html.to_string(), None);

        assert!(result.is_ok());
    }

    #[test]
    fn test_url_resolution() {
        let html = r#"
            <article>
                <h1>Links Test</h1>
                <p>Check out <a href="/about">this link</a> and <img src="/image.png" alt="test"></p>
            </article>
        "#;

        let base_url = Url::parse("https://example.com/page").unwrap();
        let input = HtmlInput;
        let result = input.process_text(html.to_string(), Some(&base_url));

        assert!(result.is_ok());
    }
}
