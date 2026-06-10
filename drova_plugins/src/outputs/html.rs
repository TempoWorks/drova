use dalet::types::{Align, Body, HeadingLevel, ListStyle, Page, TableRows, Tag};
use drova_sdk::requester::{Error, OutputData, OutputHandler};

pub struct HtmlOutput;

impl OutputHandler for HtmlOutput {
    fn process_page(&self, page: Page) -> Result<OutputData, Error> {
        Ok(OutputData::Text(render_page(&page)))
    }
}

fn render_page(page: &Page) -> String {
    let mut out = String::new();

    for tag in &page.body {
        render_tag(&mut out, tag);
        out.push('\n');
    }

    out
}

fn render_tag(out: &mut String, tag: &Tag) {
    match tag {
        Tag::Element { body } => render_body(out, body),
        Tag::Heading { body, heading } => {
            let level = heading_level(heading);
            out.push_str("<h");
            out.push(char::from(b'0' + level));
            out.push('>');
            push_escaped(out, body);
            out.push_str("</h");
            out.push(char::from(b'0' + level));
            out.push('>');
        }
        Tag::Paragraph { body } => {
            out.push_str("<p>");
            render_body(out, body);
            out.push_str("</p>");
        }
        Tag::Link { body, dref } => render_link(out, "a", dref, body, Some("_blank")),
        Tag::NavLink { body, dref } => render_link(out, "a", dref, body, None),
        Tag::Button { body, dref } => {
            render_link(out, "a class=\"button\"", dref, body, Some("_blank"))
        }
        Tag::NavButton { body, dref } => render_link(out, "a class=\"button\"", dref, body, None),
        Tag::Image { src, alt } => {
            out.push_str("<img src=\"");
            push_attr(out, src);
            out.push_str("\" alt=\"");
            push_attr(out, alt.as_deref().unwrap_or_default());
            out.push_str("\">");
        }
        Tag::Table { body } => render_table(out, body),
        Tag::List { body, style } => render_list(out, body, style),
        Tag::Bold { body } => render_text_tag(out, "strong", body),
        Tag::Italic { body } => render_text_tag(out, "em", body),
        Tag::Strikethrough { body } => render_text_tag(out, "s", body),
        Tag::Superscript { body } => render_text_tag(out, "sup", body),
        Tag::Subscript { body } => render_text_tag(out, "sub", body),
        Tag::Mono { body } => render_text_tag(out, "code", body),
        Tag::FootLink { footnote } => {
            out.push_str("<sup><a href=\"#footnote-");
            out.push_str(&footnote.to_string());
            out.push_str("\">");
            out.push_str(&footnote.to_string());
            out.push_str("</a></sup>");
        }
        Tag::FootNote { body, footnote } => {
            out.push_str("<aside id=\"footnote-");
            out.push_str(&footnote.to_string());
            out.push_str("\"><sup>");
            out.push_str(&footnote.to_string());
            out.push_str("</sup> ");
            push_escaped(out, body);
            out.push_str("</aside>");
        }
        Tag::Anchor { id } => {
            out.push_str("<span id=\"");
            push_attr(out, id);
            out.push_str("\"></span>");
        }
        Tag::BlockQuote { body } => {
            out.push_str("<blockquote>");
            render_body(out, body);
            out.push_str("</blockquote>");
        }
        Tag::Code { body, language } => {
            out.push_str("<pre><code");
            if let Some(language) = language {
                out.push_str(" class=\"language-");
                push_attr(out, language);
                out.push('"');
            }
            out.push('>');
            push_escaped(out, body);
            out.push_str("</code></pre>");
        }
        Tag::InlineCode { body, language } => {
            out.push_str("<code");
            if let Some(language) = language {
                out.push_str(" class=\"language-");
                push_attr(out, language);
                out.push('"');
            }
            out.push('>');
            push_escaped(out, body);
            out.push_str("</code>");
        }
        Tag::Block { body } => render_container(out, "section", body),
        Tag::Flex {
            body,
            wrap,
            align_x,
            align_y,
        } => render_layout(out, "flex", *wrap, align_x, align_y, body),
        Tag::Grid {
            body,
            align_x,
            align_y,
        } => render_layout(out, "grid", false, align_x, align_y, body),
        Tag::Disclosure { body, title } => {
            out.push_str("<details>");
            if let Some(title) = title {
                out.push_str("<summary>");
                push_escaped(out, title);
                out.push_str("</summary>");
            }
            render_body(out, body);
            out.push_str("</details>");
        }
        Tag::Carousel { body } => render_container(out, "div class=\"carousel\"", body),
        Tag::Variable { idx } => {
            out.push_str("<var>");
            out.push_str(&idx.to_string());
            out.push_str("</var>");
        }
        Tag::HorizontalBreak => out.push_str("<hr>"),
    }
}

fn render_body(out: &mut String, body: &Body) {
    match body {
        Body::Text(text) => push_escaped(out, text),
        Body::Tags(tags) => {
            for tag in tags {
                render_tag(out, tag);
            }
        }
    }
}

fn render_link(
    out: &mut String,
    tag_start: &str,
    dref: &str,
    body: &Option<Body>,
    target: Option<&str>,
) {
    out.push('<');
    out.push_str(tag_start);
    out.push_str(" href=\"");
    push_attr(out, dref);
    out.push('"');
    if let Some(target) = target {
        out.push_str(" target=\"");
        push_attr(out, target);
        out.push('"');
    }
    out.push('>');
    match body {
        Some(body) => render_body(out, body),
        None => push_escaped(out, dref),
    }
    out.push_str("</a>");
}

fn render_table(out: &mut String, rows: &[TableRows]) {
    out.push_str("<table>");
    for row in rows {
        match row {
            TableRows::Primary(tags) => {
                out.push_str("<tr>");
                for tag in tags {
                    out.push_str("<th>");
                    render_tag(out, tag);
                    out.push_str("</th>");
                }
                out.push_str("</tr>");
            }
            TableRows::Default(tags) => {
                out.push_str("<tr>");
                for tag in tags {
                    out.push_str("<td>");
                    render_tag(out, tag);
                    out.push_str("</td>");
                }
                out.push_str("</tr>");
            }
        }
    }
    out.push_str("</table>");
}

fn render_list(out: &mut String, body: &[Tag], style: &ListStyle) {
    let tag = match style {
        ListStyle::Decimal => "ol",
        ListStyle::Disc | ListStyle::None => "ul",
    };
    out.push('<');
    out.push_str(tag);
    if matches!(style, ListStyle::None) {
        out.push_str(" class=\"list-none\"");
    }
    out.push('>');
    for item in body {
        out.push_str("<li>");
        render_tag(out, item);
        out.push_str("</li>");
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn render_text_tag(out: &mut String, tag: &str, body: &str) {
    out.push('<');
    out.push_str(tag);
    out.push('>');
    push_escaped(out, body);
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn render_container(out: &mut String, tag_start: &str, body: &[Tag]) {
    out.push('<');
    out.push_str(tag_start);
    out.push('>');
    for tag in body {
        render_tag(out, tag);
    }
    out.push_str("</");
    out.push_str(tag_start.split_whitespace().next().unwrap_or(tag_start));
    out.push('>');
}

fn render_layout(
    out: &mut String,
    class_name: &str,
    wrap: bool,
    align_x: &Option<Align>,
    align_y: &Option<Align>,
    body: &[Tag],
) {
    out.push_str("<div class=\"");
    out.push_str(class_name);
    if wrap {
        out.push_str(" wrap");
    }
    if let Some(align) = align_x {
        out.push_str(" align-x-");
        out.push_str(align_class(align));
    }
    if let Some(align) = align_y {
        out.push_str(" align-y-");
        out.push_str(align_class(align));
    }
    out.push_str("\">");
    for tag in body {
        render_tag(out, tag);
    }
    out.push_str("</div>");
}

fn heading_level(level: &HeadingLevel) -> u8 {
    match level {
        HeadingLevel::One => 1,
        HeadingLevel::Two => 2,
        HeadingLevel::Three => 3,
        HeadingLevel::Four => 4,
        HeadingLevel::Five => 5,
        HeadingLevel::Six => 6,
    }
}

fn align_class(align: &Align) -> &'static str {
    match align {
        Align::Start => "start",
        Align::Center => "center",
        Align::End => "end",
    }
}

fn push_escaped(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}

fn push_attr(out: &mut String, text: &str) {
    push_escaped(out, text);
}
