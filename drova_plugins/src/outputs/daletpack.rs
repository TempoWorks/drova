use dalet::daletpack;
use dalet::types::Page;
use drova_sdk::requester::{Error, OutputData, OutputHandler};

pub struct DaletpackOutput;

impl OutputHandler for DaletpackOutput {
    fn process_page(&self, page: Page) -> Result<OutputData, Error> {
        let bytes =
            daletpack::serialize(page).map_err(|e| Error::ParserError(format!("{:?}", e)))?;

        Ok(OutputData::Bytes(bytes))
    }
}
