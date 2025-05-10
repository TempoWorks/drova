use std::collections::HashMap;

use dalet::typed::{ListStyle, Page, TableRows, Tag, Text};
use drova_sdk::{Error, Input};
use markdown::mdast::Node;
use url::Url;

pub struct MarkdownInput;

impl Input for MarkdownInput {
    fn process_text(&self, s: String, _: Option<&Url>) -> Result<Page, Error> {
        let ast = markdown::to_mdast(&s, &markdown::ParseOptions::gfm())
            .map_err(|_| Error::InvalidSyntax)?;

        let mut page: Page = Page {
            title: None,
            description: None,
            body: vec![],
            variables: None,
        };

        let mut foot_count: u64 = 0;
        let mut footnotes: HashMap<String, u64> = HashMap::new();

        match ast {
            Node::Root(root) => {
                manage_foot_links(&mut foot_count, &mut footnotes, &root.children);

                for node in root.children {
                    let tag_q = convert_node(&mut page, &mut foot_count, &mut footnotes, node);

                    match tag_q {
                        Ok(tag) => page.body.push(tag),
                        Err(e) => match e {
                            Error::InvalidSyntax => {}
                            _ => Err(e)?,
                        },
                    }
                }
            }

            _ => {
                Err(Error::InvalidSyntax)?;
            }
        }

        Ok(page)
    }

    fn process_bytes(&self, _: Vec<u8>, _: Option<&Url>) -> Result<Page, Error> {
        Err(Error::UnsupportedInput)
    }
}

fn manage_foot_links(
    foot_count: &mut u64,
    footnotes: &mut HashMap<String, u64>,
    nodes: &Vec<Node>,
) {
    for node in nodes {
        match node {
            Node::FootnoteDefinition(n) => {
                footnotes.insert(n.identifier.clone(), *foot_count);

                *foot_count += 1;
            }
            _ => {}
        }
    }
}

fn convert_node(
    page: &mut Page,
    foot_count: &mut u64,
    footnotes: &mut HashMap<String, u64>,
    node: Node,
) -> Result<Tag, Error> {
    match node {
        Node::Blockquote(n) => Ok(Tag::BlockQuote {
            body: convert_nodes(page, foot_count, footnotes, n.children)?.into(),
        }),

        Node::Break(_) => Ok(Tag::LineBreak),

        Node::InlineCode(n) => Ok(Tag::Element {
            body: n.value.into(),
        }),

        Node::InlineMath(n) => Ok(Tag::Element {
            body: n.value.into(),
        }),

        Node::Delete(n) => Ok(Tag::Strikethrough {
            body: nodes_to_text(n.children)?,
        }),

        Node::Emphasis(n) => Ok(Tag::Italic {
            body: nodes_to_text(n.children)?,
        }),

        Node::Image(n) => Ok(Tag::Image {
            src: n.url,
            alt: Some(n.alt),
        }),

        Node::Link(n) => Ok(Tag::Link {
            dref: n.url,
            body: Some(nodes_to_text(n.children)?.into()),
        }),

        Node::Strong(n) => Ok(Tag::Bold {
            body: nodes_to_text(n.children)?,
        }),

        Node::Text(n) => Ok(Tag::Element {
            body: n.value.into(),
        }),

        Node::Code(n) => Ok(Tag::Code {
            body: n.value.into(),
            language: n.lang,
        }),

        Node::Math(n) => Ok(Tag::Paragraph {
            body: n.value.into(),
        }),

        Node::Definition(n) => Ok(Tag::Anchor { id: n.identifier }),

        Node::FootnoteDefinition(n) => Ok(Tag::FootNote {
            body: nodes_to_text(n.children)?.into(),
            footnote: *footnotes.get(&n.identifier).unwrap(),
        }),

        Node::FootnoteReference(n) => Ok(Tag::FootLink {
            footnote: *footnotes
                .get(&n.identifier)
                .ok_or(Error::ParserError(format!(
                    "FootLink ({}) does not have parent FootNote",
                    n.identifier
                )))?,
        }),

        Node::List(n) => Ok(Tag::List {
            body: convert_nodes(page, foot_count, footnotes, n.children)?.into(),
            style: if n.ordered {
                ListStyle::Decimal
            } else {
                ListStyle::Disc
            },
        }),
        Node::ListItem(n) => Ok(Tag::Element {
            body: convert_nodes(page, foot_count, footnotes, n.children)?.into(),
        }),

        Node::Table(n) => {
            let mut rows: Vec<TableRows> = vec![];
            let mut primary_been = false;

            for row in n.children {
                match row {
                    Node::TableCell(n) => {
                        if primary_been {
                            rows.push(TableRows::Default(
                                convert_nodes(page, foot_count, footnotes, n.children)?.into(),
                            ));
                        } else {
                            primary_been = true;
                            rows.push(TableRows::Primary(
                                convert_nodes(page, foot_count, footnotes, n.children)?.into(),
                            ));
                        }
                    }
                    _ => Err(Error::ParserError("Invalid tag in Table".into()))?,
                }
            }

            Ok(Tag::Table { body: rows })
        }

        Node::TableCell(n) => Ok(Tag::Element {
            body: convert_nodes(page, foot_count, footnotes, n.children)?.into(),
        }),

        Node::ThematicBreak(_) => Ok(Tag::HorizontalBreak),
        Node::Heading(n) => Ok(Tag::Heading {
            body: nodes_to_text(n.children)?,
            heading: n
                .depth
                .try_into()
                .unwrap_or(dalet::typed::HeadingLevel::Six),
        }),
        Node::Paragraph(n) => Ok(Tag::Paragraph {
            body: convert_nodes(page, foot_count, footnotes, n.children)?.into(),
        }),

        Node::TableRow(_) => Err(Error::InvalidSyntax),

        // Unsupported
        Node::Root(_)
        | Node::Html(_)
        | Node::Toml(_)
        | Node::Yaml(_)
        | Node::LinkReference(_)
        | Node::ImageReference(_)
        | Node::MdxTextExpression(_)
        | Node::MdxjsEsm(_)
        | Node::MdxJsxFlowElement(_)
        | Node::MdxJsxTextElement(_)
        | Node::MdxFlowExpression(_) => Err(Error::InvalidSyntax),
    }
}

fn convert_nodes(
    page: &mut Page,
    foot_count: &mut u64,
    footnotes: &mut HashMap<String, u64>,
    nodes: Vec<Node>,
) -> Result<Vec<Tag>, Error> {
    nodes
        .into_iter()
        .map(|node| convert_node(page, foot_count, footnotes, node))
        .collect()
}

fn nodes_to_text(nodes: Vec<Node>) -> Result<Text, Error> {
    let mut output = "".to_owned();

    for node in nodes {
        match node {
            Node::Break(_) => output.push('\n'),
            Node::ThematicBreak(_) => output.push(' '),
            Node::Math(n) => output.push_str(&n.value),
            Node::Image(n) => output.push_str(&n.alt),
            Node::InlineCode(n) => output.push_str(&n.value),
            Node::InlineMath(n) => output.push_str(&n.value),
            Node::Text(n) => output.push_str(&n.value),
            Node::Code(n) => output.push_str(&n.value),
            Node::Definition(n) => output.push_str(&n.identifier),
            Node::ImageReference(n) => output.push_str(&n.alt),
            Node::FootnoteReference(n) => output.push_str(&n.identifier),
            Node::Blockquote(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::FootnoteDefinition(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::List(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::Emphasis(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::Delete(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::Link(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::LinkReference(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::Strong(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::Heading(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::Table(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::TableRow(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::TableCell(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::ListItem(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::Paragraph(n) => output.push_str(&nodes_to_text(n.children)?),
            Node::MdxTextExpression(_) => Err(Error::InvalidSyntax)?,
            Node::Root(_) => Err(Error::InvalidSyntax)?,
            Node::Html(_) => Err(Error::InvalidSyntax)?,
            Node::MdxjsEsm(_) => Err(Error::InvalidSyntax)?,
            Node::MdxJsxFlowElement(_) => Err(Error::InvalidSyntax)?,
            Node::MdxJsxTextElement(_) => Err(Error::InvalidSyntax)?,
            Node::MdxFlowExpression(_) => Err(Error::InvalidSyntax)?,
            Node::Toml(_) => Err(Error::InvalidSyntax)?,
            Node::Yaml(_) => Err(Error::InvalidSyntax)?,
        };
    }

    Ok(output)
}
