use dalet::types::{Page, Tag};
use drova_sdk::requester::{Error, InputHandler};

pub struct TextInput;

impl InputHandler for TextInput {
    fn process_text(&self, data: String, _: Option<&url::Url>) -> Result<Page, Error> {
        let title = truncate(&data, 20);
        let description = truncate(&data, 100);

        Ok(Page {
            title: Some(title.into()),
            description: Some(description.into()),
            body: vec![Tag::Mono { body: data.into() }],
            variables: None,
        })
    }

    fn process_bytes(&self, _: Vec<u8>, _: Option<&url::Url>) -> Result<Page, Error> {
        Err(Error::UnsupportedInput)
    }
}

pub fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
