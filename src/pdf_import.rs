//! Pull card-shaped images out of an existing PDF.
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage};
use lopdf::{content::Content, Dictionary, Document, Object, ObjectId};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};

pub struct Found {
    pub img: DynamicImage,
    pub copies: usize,
}

fn filters(d: &Dictionary) -> Vec<Vec<u8>> {
    match d.get(b"Filter") {
        Ok(Object::Name(n)) => vec![n.clone()],
        Ok(Object::Array(a)) => a
            .iter()
            .filter_map(|o| o.as_name().ok().map(|n| n.to_vec()))
            .collect(),
        _ => vec![],
    }
}

/// Decode an image XObject; None for anything we don't handle (CMYK, indexed, JPX, 1-bit, ...).
fn decode(doc: &Document, id: ObjectId) -> Option<DynamicImage> {
    let s = doc.get_object(id).ok()?.as_stream().ok()?;
    if s.dict.get(b"Subtype").ok()?.as_name().ok()? != b"Image" {
        return None;
    }
    if let Ok(Object::Array(cs)) = s.dict.get(b"ColorSpace") {
        if cs.first().and_then(|o| o.as_name().ok()) == Some(b"Indexed".as_slice()) {
            return None;
        }
    }
    let f = filters(&s.dict);
    match f.as_slice() {
        [x] if x == b"DCTDecode" => {
            image::load_from_memory_with_format(&s.content, ImageFormat::Jpeg).ok()
        }
        [] | [_] => {
            let (w, h) = (
                s.dict.get(b"Width").ok()?.as_i64().ok()? as u32,
                s.dict.get(b"Height").ok()?.as_i64().ok()? as u32,
            );
            let bpc = s
                .dict
                .get(b"BitsPerComponent")
                .and_then(|o| o.as_i64())
                .unwrap_or(8);
            if bpc != 8 || w == 0 || h == 0 {
                return None;
            }
            let data = if f.is_empty() {
                s.content.clone()
            } else {
                s.decompressed_content().ok()?
            };
            match data.len() / (w as usize * h as usize) {
                3 => RgbImage::from_raw(w, h, data).map(DynamicImage::ImageRgb8),
                1 => GrayImage::from_raw(w, h, data).map(DynamicImage::ImageLuma8),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The page's XObject resource dictionary (walking up /Parent for inherited resources).
fn xobjects(doc: &Document, mut id: ObjectId) -> Option<&Dictionary> {
    loop {
        let d = doc.get_dictionary(id).ok()?;
        if let Ok(r) = d.get(b"Resources") {
            let r = doc.dereference(r).ok()?.1.as_dict().ok()?;
            return doc
                .dereference(r.get(b"XObject").ok()?)
                .ok()?
                .1
                .as_dict()
                .ok();
        }
        id = d.get(b"Parent").ok()?.as_reference().ok()?;
    }
}

fn is_card(w: u32, h: u32, card_w: f32, card_h: f32, tol: f32) -> bool {
    let r = w.min(h) as f32 / w.max(h) as f32;
    let target = card_w.min(card_h) / card_w.max(card_h);
    (r / target - 1.0).abs() <= tol
}

/// Extract card-like images, in order of first appearance. `tol` is the relative
/// aspect-ratio tolerance (0.04 = 4%).
pub fn extract(bytes: &[u8], card_w: f32, card_h: f32, tol: f32) -> Result<Vec<Found>, String> {
    let doc = Document::load_mem(bytes).map_err(|e| e.to_string())?;

    // How many times each image object is painted, in first-use order.
    let mut uses: HashMap<ObjectId, usize> = HashMap::new();
    let mut order = Vec::new();
    for (_, page) in doc.get_pages() {
        let Some(xo) = xobjects(&doc, page) else {
            continue;
        };
        let Ok(raw) = doc.get_page_content(page) else {
            continue;
        };
        let Ok(content) = Content::decode(&raw) else {
            continue;
        };
        for op in content.operations.iter().filter(|o| o.operator == "Do") {
            let Some(name) = op.operands.first().and_then(|o| o.as_name().ok()) else {
                continue;
            };
            let Ok(id) = xo.get(name).and_then(|o| o.as_reference()) else {
                continue;
            };
            if !uses.contains_key(&id) {
                order.push(id);
            }
            *uses.entry(id).or_default() += 1;
        }
    }

    let portrait = card_h >= card_w;
    let mut out: Vec<Found> = Vec::new();
    let mut seen: HashMap<u64, usize> = HashMap::new();
    for id in order {
        let Some(img) = decode(&doc, id) else {
            continue;
        };
        if img.width().min(img.height()) < 100
            || !is_card(img.width(), img.height(), card_w, card_h, tol)
        {
            continue;
        }
        let img = if (img.height() >= img.width()) != portrait {
            img.rotate270()
        } else {
            img
        };
        let mut h = DefaultHasher::new();
        img.as_bytes().hash(&mut h);
        match seen.get(&h.finish()) {
            Some(&i) => out[i].copies += uses[&id],
            None => {
                seen.insert(h.finish(), out.len());
                out.push(Found {
                    img,
                    copies: uses[&id],
                });
            }
        }
    }
    if out.is_empty() {
        return Err("no card-shaped images found in this PDF".into());
    }
    Ok(out)
}
