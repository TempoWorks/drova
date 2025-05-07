use mime::Mime;

pub fn mime_to_str<'a>(mime: Mime) -> String {
    format!("{}/{}", mime.type_().as_str(), mime.subtype().as_str())
}
