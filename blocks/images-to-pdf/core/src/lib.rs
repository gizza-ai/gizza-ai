//! gizza-ai/images-to-pdf core — pure-Rust "combine images into one PDF" shared
//! by the chat skill block + CLI. No wafer/wasm-bindgen deps.
//!
//! Each input image is decoded to RGB (any common raster format), embedded as a
//! Flate-compressed image XObject, and placed on its own page sized to the image
//! (1px = 1pt). Pages are assembled with lopdf and serialized.

use std::io::Write;

use flate2::{write::ZlibEncoder, Compression};
use lopdf::{dictionary, Document, Object, Stream};

/// Build a PDF from the given image byte buffers (one image per page, in order).
pub fn images_to_pdf(images: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if images.is_empty() {
        return Err("need at least one image".into());
    }
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids: Vec<Object> = Vec::with_capacity(images.len());

    for (i, bytes) in images.iter().enumerate() {
        let rgb = image::load_from_memory(bytes)
            .map_err(|e| format!("image #{} failed to decode: {e}", i + 1))?
            .to_rgb8();
        let (w, h) = (rgb.width(), rgb.height());

        // Flate-compress the raw RGB samples for the image XObject.
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(rgb.as_raw())
            .and_then(|_| enc.flush())
            .map_err(|e| format!("compress image #{}: {e}", i + 1))?;
        let compressed = enc.finish().map_err(|e| format!("compress image #{}: {e}", i + 1))?;

        let mut img_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w as i64,
                "Height" => h as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "FlateDecode",
            },
            compressed,
        );
        // We set Filter ourselves and pre-compressed the data, so stop lopdf from
        // trying to (re)compress this stream on save.
        img_stream.allows_compression = false;
        let img_id = doc.add_object(img_stream);

        // Content stream: paint the image to fill the whole page.
        let content = format!("q {w} 0 0 {h} 0 0 cm /Img Do Q");
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));

        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Img" => img_id },
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), (w as i64).into(), (h as i64).into()],
            "Contents" => content_id,
            "Resources" => resources_id,
        });
        page_ids.push(page_id.into());
    }

    let count = page_ids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => count,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn png(w: u32, h: u32, color: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb(color));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).expect("output should parse").get_pages().len()
    }

    #[test]
    fn one_image_one_page() {
        let pdf = images_to_pdf(&[png(20, 30, [255, 0, 0])]).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn three_images_three_pages() {
        let imgs = vec![png(10, 10, [255, 0, 0]), png(20, 20, [0, 255, 0]), png(30, 30, [0, 0, 255])];
        let pdf = images_to_pdf(&imgs).unwrap();
        assert_eq!(page_count(&pdf), 3);
    }

    #[test]
    fn mediabox_matches_image_size() {
        let pdf = images_to_pdf(&[png(123, 45, [10, 20, 30])]).unwrap();
        let doc = Document::load_mem(&pdf).unwrap();
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        let mb = doc.get_object(page_id).unwrap().as_dict().unwrap().get(b"MediaBox").unwrap();
        let arr = mb.as_array().unwrap();
        assert_eq!(arr[2].as_i64().unwrap(), 123);
        assert_eq!(arr[3].as_i64().unwrap(), 45);
    }

    #[test]
    fn empty_errors() {
        assert!(images_to_pdf(&[]).is_err());
    }

    #[test]
    fn bad_image_errors() {
        assert!(images_to_pdf(&[b"not an image".to_vec()]).is_err());
    }
}
