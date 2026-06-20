//! gizza-ai/qr-decode core — read and decode the data in a QR code image.
//! Pure-Rust (`image` for decoding + `quircs` for QR recognition). No
//! wafer/wasm-bindgen deps.
//!
//! Decodes every QR code found in the image (an image may contain more than one)
//! and returns the textual content of each, in detection order.

/// Decode all QR codes in `image_bytes`. Returns each code's decoded text.
/// Errors if the image can't be decoded or contains no readable QR code.
pub fn run(image_bytes: &[u8]) -> Result<Vec<String>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let luma = img.to_luma8();
    let (w, h) = (luma.width() as usize, luma.height() as usize);
    if w == 0 || h == 0 {
        return Err("image has zero size".into());
    }

    let mut decoder = quircs::Quirc::default();
    let codes = decoder.identify(w, h, &luma);

    let mut out = Vec::new();
    let mut last_err: Option<String> = None;
    for code in codes {
        match code {
            Ok(c) => match c.decode() {
                Ok(decoded) => {
                    out.push(String::from_utf8_lossy(&decoded.payload).into_owned())
                }
                Err(e) => last_err = Some(e.to_string()),
            },
            Err(e) => last_err = Some(e.to_string()),
        }
    }

    if out.is_empty() {
        return Err(match last_err {
            Some(e) => format!("found a QR pattern but could not decode it: {e}"),
            None => "no QR code found in the image".to_string(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;
    use qrcode::QrCode;

    /// Render `text` to a PNG QR image (scaled up so detection is reliable).
    fn qr_png(text: &str) -> Vec<u8> {
        let code = QrCode::new(text.as_bytes()).unwrap();
        let img = code
            .render::<Luma<u8>>()
            .min_dimensions(300, 300)
            .quiet_zone(true)
            .build();
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn decodes_url() {
        let png = qr_png("https://gizza.ai");
        let out = run(&png).unwrap();
        assert_eq!(out, vec!["https://gizza.ai".to_string()]);
    }

    #[test]
    fn decodes_longer_text() {
        let text = "The quick brown fox jumps over the lazy dog 0123456789";
        let png = qr_png(text);
        let out = run(&png).unwrap();
        assert_eq!(out, vec![text.to_string()]);
    }

    #[test]
    fn errors_on_non_image() {
        assert!(run(b"not an image").is_err());
    }

    #[test]
    fn errors_when_no_qr() {
        // A blank white image has no QR code.
        let img = image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(
            64,
            64,
            Luma([255u8]),
        ));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        assert!(run(&buf.into_inner()).is_err());
    }
}
