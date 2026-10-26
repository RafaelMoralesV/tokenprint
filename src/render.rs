//! Image preparation (black corners, rotation) and PDF export.
use crate::layout::{slot, Grid, Params};
use image::{codecs::jpeg::JpegEncoder, imageops, DynamicImage, RgbImage};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};

/// Flatten transparency (onto black when `black_corners`, else white), optionally paint the
/// areas outside the rounded corners black, and optionally rotate 90° clockwise.
pub fn prepare(img: &DynamicImage, black_corners: bool, radius_mm: f32, card_w: f32, card_h: f32, rotate: bool) -> RgbImage {
    let mut rgba = img.to_rgba8();
    let bg = if black_corners { 0u32 } else { 255 };
    for p in rgba.pixels_mut() {
        let a = p[3] as u32;
        for c in 0..3 {
            p[c] = ((p[c] as u32 * a + bg * (255 - a)) / 255) as u8;
        }
        p[3] = 255;
    }
    let (w, h) = rgba.dimensions();
    if black_corners && radius_mm > 0.0 {
        let (rx, ry) = (radius_mm / card_w * w as f32, radius_mm / card_h * h as f32);
        for cy in 0..(ry.ceil() as u32).min(h) {
            for cx in 0..(rx.ceil() as u32).min(w) {
                let (dx, dy) = ((rx - cx as f32 - 0.5) / rx, (ry - cy as f32 - 0.5) / ry);
                if dx * dx + dy * dy > 1.0 {
                    for (x, y) in [(cx, cy), (w - 1 - cx, cy), (cx, h - 1 - cy), (w - 1 - cx, h - 1 - cy)] {
                        rgba.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
                    }
                }
            }
        }
    }
    let rgb = DynamicImage::ImageRgba8(rgba).to_rgb8();
    if rotate { imageops::rotate90(&rgb) } else { rgb }
}

/// Build a PDF whose page size equals the canvas. Each image is embedded once and reused for
/// every copy. `images` is (prepared image, copies); copies run in list order, row by row.
pub fn export_pdf(p: &Params, g: &Grid, images: &[(RgbImage, usize)]) -> Result<Vec<u8>, String> {
    let per = g.per_page();
    if per == 0 {
        return Err("cards do not fit on the canvas".into());
    }
    let cards: Vec<usize> = images.iter().enumerate().flat_map(|(i, (_, n))| std::iter::repeat(i).take(*n)).collect();
    if cards.is_empty() {
        return Err("nothing to print".into());
    }
    let k = 72.0 / 25.4;
    let (pw, ph) = (p.page_w * k, p.page_h * k);
    let mut next = 3;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let (catalog, tree) = (Ref::new(1), Ref::new(2));
    let mut pdf = Pdf::new();

    let mut refs: Vec<(String, Ref)> = Vec::new();
    for (i, (img, _)) in images.iter().enumerate() {
        let mut jpg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpg, 95).encode_image(img).map_err(|e| e.to_string())?;
        let id = alloc();
        let mut x = pdf.image_xobject(id, &jpg);
        x.width(img.width() as i32);
        x.height(img.height() as i32);
        x.filter(Filter::DctDecode);
        x.color_space().device_rgb();
        x.bits_per_component(8);
        x.finish();
        refs.push((format!("Im{i}"), id));
    }

    let page_count = cards.len().div_ceil(per);
    let mut page_ids = Vec::new();
    for pg in 0..page_count {
        let (page_id, content_id) = (alloc(), alloc());
        page_ids.push(page_id);
        let mut c = Content::new();
        for (s, &img) in cards[pg * per..].iter().take(per).enumerate() {
            let (x, y) = slot(p, g, s);
            c.save_state();
            c.transform([g.cw * k, 0.0, 0.0, g.ch * k, x * k, ph - (y + g.ch) * k]);
            c.x_object(Name(refs[img].0.as_bytes()));
            c.restore_state();
        }
        pdf.stream(content_id, &c.finish());
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, pw, ph)).parent(tree).contents(content_id);
        let mut res = page.resources();
        let mut xo = res.x_objects();
        for (n, r) in &refs {
            xo.pair(Name(n.as_bytes()), *r);
        }
        xo.finish();
        res.finish();
        page.finish();
    }
    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree).kids(page_ids).count(page_count as i32);
    Ok(pdf.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::grid;
    use image::{Rgba, RgbaImage};

    #[test]
    fn corners_go_black_centre_stays() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(630, 880, Rgba([255, 255, 255, 255])));
        let out = prepare(&img, true, 3.2, 63.0, 88.0, false);
        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 0]);
        assert_eq!(out.get_pixel(629, 879).0, [0, 0, 0]);
        assert_eq!(out.get_pixel(315, 440).0, [255, 255, 255]);
        let off = prepare(&img, false, 3.2, 63.0, 88.0, false);
        assert_eq!(off.get_pixel(0, 0).0, [255, 255, 255]);
    }

    #[test]
    fn rotation_swaps_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(63, 88));
        assert_eq!(prepare(&img, false, 0.0, 63.0, 88.0, true).dimensions(), (88, 63));
    }

    #[test]
    fn exports_multi_page_pdf_with_canvas_size() {
        let p = Params::default();
        let g = grid(&p);
        let a = RgbImage::from_pixel(63, 88, image::Rgb([200, 30, 30]));
        let b = RgbImage::from_pixel(63, 88, image::Rgb([30, 30, 200]));
        let bytes = export_pdf(&p, &g, &[(a, 15), (b, 10)]).unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(bytes.starts_with(b"%PDF"));
        assert!(s.contains("/Count 2"), "25 cards at 21/page = 2 pages");
        assert!(s.contains("1296 864"), "18x12 in = 1296x864 pt");
        std::fs::write(std::env::temp_dir().join("tokenprint_test.pdf"), &bytes).unwrap();
    }
}
