use dalet::daletpack;
use dalet::types::Page;
use drova_sdk::requester::{Error, InputHandler};

pub struct DaletpackInput;

impl InputHandler for DaletpackInput {
    fn process_text(&self, _: String, _: Option<&url::Url>) -> Result<Page, Error> {
        Err(Error::UnsupportedInput)
    }

    fn process_bytes(&self, data: Vec<u8>, _: Option<&url::Url>) -> Result<Page, Error> {
        let decompressed =
            daletpack::decompress(&data).map_err(|e| Error::ParserError(format!("{:?}", e)))?;

        let page: Page = daletpack::deserialize(&decompressed)
            .map_err(|e| Error::ParserError(format!("{:?}", e)))?;

        Ok(page)
    }
}
