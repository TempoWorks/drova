use dalet::typed::Page;
use std::collections::HashMap;
use std::sync::Arc;

// Define response types
#[derive(Debug)]
pub enum Response<'a> {
    TextOutput(&'a str),
    BitsOutput(&'a [u8]),
    GetInputRequired,
    NotFound,
    // Add other response types as needed
}

#[derive(Debug)]
pub enum Error {}

pub trait Protocol {
    fn fetch(&self, url: &str) -> Result<(Response, String), Error>;
}

pub trait Input {
    fn process(&self, response: Response) -> Result<Page, Error>;
}

pub struct Core<'a> {
    protocols: HashMap<&'a str, Arc<dyn Protocol>>,
    inputs: HashMap<&'a str, Arc<dyn Input>>,
}

impl Default for Core<'_> {
    fn default() -> Self {
        Self {
            protocols: HashMap::new(),
            inputs: HashMap::new(),
        }
    }
}

pub struct CoreBuilder<'a> {
    core: Core<'a>,
}

impl Default for CoreBuilder<'_> {
    fn default() -> Self {
        Self {
            core: Core::default(),
        }
    }
}

impl<'a> CoreBuilder<'a> {
    pub fn plugins<R>(mut self, registrars: Vec<R>) -> Self
    where
        R: Fn(&mut CoreBuilder),
    {
        for registrar in registrars {
            registrar(&mut self);
        }

        self
    }

    pub fn protocol(mut self, schema: &'a str, protocol: Arc<dyn Protocol>) -> Self {
        self.core.protocols.insert(schema, protocol);
        self
    }

    pub fn input(mut self, ty: &'a str, input: Arc<dyn Input>) -> Self {
        self.core.inputs.insert(ty, input);
        self
    }

    pub fn build(self) -> Core<'a> {
        self.core
    }
}

fn main() {
    let core = CoreBuilder::default()
        .plugins(vec![|app: &mut CoreBuilder| {}])
        .build();

    // core.process("gemini://example.com")
    //     .unwrap()
    //     .output("dalet")
    //     .unwrap();
}
