use dalet::typed::{
    HeadingLevel, ListStyle, Page,
    Tag::{self, *},
};
use drova_sdk::{Error, Input};
use url::Url;

pub struct GemtextInput;

impl Input for GemtextInput {
    fn process_text(&self, s: String, _: Option<&Url>) -> Result<Page, Error> {
        let mut page: Vec<Tag> = Vec::new();
        let mut preformatted = false;
        let mut preformatted_text: Vec<String> = Vec::new();

        let mut list_before = false;
        let mut list: Vec<Tag> = Vec::new();

        let mut title: Option<String> = None;

        for line in s.lines() {
            let mut line = line.trim().to_owned();

            if preformatted && !line.starts_with("```") {
                preformatted_text.push(line);
            } else if list_before && !line.starts_with("* ") {
                page.push(Tag::List {
                    body: list.clone(),
                    style: ListStyle::Disc,
                });
                list_before = false;
                list.clear();
            } else if line.starts_with("=>") {
                let body = line.split_off(2);
                let mut body = body.trim().splitn(2, char::is_whitespace);

                let url = body.next().ok_or(Error::InvalidSyntax)?.trim();

                match body.next() {
                    Some(label) => page.push(Paragraph {
                        body: vec![NavLink {
                            body: Some(label.trim().into()),
                            dref: url.into(),
                        }]
                        .into(),
                    }),
                    None => page.push(Paragraph {
                        body: vec![NavLink {
                            body: None,
                            dref: url.into(),
                        }]
                        .into(),
                    }),
                };
            } else if line.starts_with("# ") {
                let body = line.split_off(2).trim().to_owned();

                if title == None {
                    title = Some(body.clone())
                }

                page.push(Heading {
                    body,
                    heading: HeadingLevel::One,
                });
            } else if line.starts_with("## ") {
                let body = line.split_off(3);
                page.push(Heading {
                    body: body.trim().into(),
                    heading: HeadingLevel::Two,
                });
            } else if line.starts_with("### ") {
                let body = line.split_off(4);
                page.push(Heading {
                    body: body.trim().into(),
                    heading: HeadingLevel::Three,
                });
            } else if line.starts_with("* ") {
                let body = line.split_off(2);
                list.push(Element { body: body.into() });
                list_before = true;
            } else if line.starts_with("> ") {
                let body = line.split_off(2);
                page.push(BlockQuote { body: body.into() });
            } else if line.starts_with("```") {
                if preformatted {
                    page.push(Code {
                        body: preformatted_text.join("\n"),
                        language: None,
                    });
                    preformatted_text.clear();
                }

                preformatted = !preformatted;
            } else if !line.is_empty() {
                page.push(Paragraph { body: line.into() });
            }
        }

        if list_before {
            page.push(List {
                body: list.clone(),
                style: ListStyle::Disc,
            });
        }

        if preformatted {
            page.push(Code {
                body: preformatted_text.join("\n"),
                language: None,
            });
        }

        Ok(Page {
            title,
            description: None,
            body: page,
            variables: None,
        })
    }

    fn process_bytes(&self, _: Vec<u8>, _: Option<&Url>) -> Result<Page, Error> {
        Err(Error::UnsupportedInput)
    }
}
