use mime::Mime;

pub fn mime_to_str<'a>(mime: Mime) -> String {
    format!("{}/{}", mime.type_().as_str(), mime.subtype().as_str())
}

pub fn decode_bytes(bytes: &[u8]) -> Result<String, std::string::FromUtf8Error> {
    // Check for BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 BOM
        return String::from_utf8(bytes[3..].to_vec());
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        // UTF-16 LE - convert to UTF-8
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return Ok(String::from_utf16_lossy(&utf16));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        // UTF-16 BE
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return Ok(String::from_utf16_lossy(&utf16));
    }

    // Try UTF-8, fallback to lossy
    String::from_utf8(bytes.to_vec()).or_else(|_| Ok(String::from_utf8_lossy(bytes).into_owned()))
}
