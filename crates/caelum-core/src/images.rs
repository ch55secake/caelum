use std::io::Read;

use base64::{engine::general_purpose::STANDARD, Engine};

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Bound both local files and authenticated provider media before crossing IPC.
pub(crate) fn data_url(reader: impl Read) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("Image is larger than 10 MB".into());
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if bytes.get(4..8) == Some(b"ftyp")
        && matches!(bytes.get(8..12), Some(b"avif" | b"avis"))
    {
        "image/avif"
    } else if bytes.starts_with(b"BM") {
        "image/bmp"
    } else if bytes.starts_with(b"\0\0\x01\0") {
        "image/x-icon"
    } else if std::str::from_utf8(&bytes).is_ok_and(|text| {
        let text = text.trim_start_matches('\u{feff}').trim_start();
        text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg"))
    }) {
        "image/svg+xml"
    } else {
        return Err("Unsupported image format".into());
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_images_and_bounds_reads() {
        assert!(data_url(&b"\x89PNG\r\n\x1a\n"[..])
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(data_url(&b"<html>not an image</html>"[..]).is_err());
        assert!(data_url(std::io::repeat(0)).unwrap_err().contains("10 MB"));
    }
}
