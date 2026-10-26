//! Grid layout maths. All lengths are millimetres.
pub const MM_PER_IN: f32 = 25.4;

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub page_w: f32,
    pub page_h: f32,
    pub margin: f32,
    pub card_w: f32,
    pub card_h: f32,
    pub gap: f32,
    pub auto_rotate: bool,
}

impl Default for Params {
    /// 18x12 in canvas, 63x88 mm cards, 0.2 mm spacing.
    fn default() -> Self {
        Self { page_w: 18.0 * MM_PER_IN, page_h: 12.0 * MM_PER_IN, margin: 0.0, card_w: 63.0, card_h: 88.0, gap: 0.2, auto_rotate: true }
    }
}

/// `cw`/`ch` are the card dimensions as placed (already swapped when `rotated`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub rotated: bool,
    pub cw: f32,
    pub ch: f32,
}

impl Grid {
    pub fn per_page(&self) -> usize {
        self.cols * self.rows
    }
}

fn fit(avail: f32, size: f32, gap: f32) -> usize {
    if size <= 0.0 || avail <= 0.0 {
        return 0;
    }
    (((avail + gap) / (size + gap)) + 1e-4).floor().max(0.0) as usize
}

/// Pick the grid; with `auto_rotate`, turn cards 90° when that fits more per sheet.
pub fn grid(p: &Params) -> Grid {
    let (aw, ah) = (p.page_w - 2.0 * p.margin, p.page_h - 2.0 * p.margin);
    let mk = |cw, ch, rotated| Grid { cols: fit(aw, cw, p.gap), rows: fit(ah, ch, p.gap), rotated, cw, ch };
    let a = mk(p.card_w, p.card_h, false);
    if p.auto_rotate {
        let b = mk(p.card_h, p.card_w, true);
        if b.per_page() > a.per_page() {
            return b;
        }
    }
    a
}

/// Top-left of slot `i` in mm from the page's top-left; the grid is centred on the page.
/// Only call with `i < g.per_page()`.
pub fn slot(p: &Params, g: &Grid, i: usize) -> (f32, f32) {
    let (c, r) = (i % g.cols, i / g.cols);
    let tw = g.cols as f32 * g.cw + (g.cols - 1) as f32 * p.gap;
    let th = g.rows as f32 * g.ch + (g.rows - 1) as f32 * p.gap;
    ((p.page_w - tw) / 2.0 + c as f32 * (g.cw + p.gap), (p.page_h - th) / 2.0 + r as f32 * (g.ch + p.gap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_spacing_fits_21_upright() {
        let g = grid(&Params::default());
        assert_eq!((g.cols, g.rows, g.rotated), (7, 3, false));
    }

    #[test]
    fn three_mm_spacing_prefers_rotated_20() {
        let g = grid(&Params { gap: 3.0, ..Params::default() });
        assert_eq!((g.cols, g.rows, g.rotated), (5, 4, true));
        let g = grid(&Params { gap: 3.0, auto_rotate: false, ..Params::default() });
        assert_eq!(g.per_page(), 18);
    }

    #[test]
    fn grid_stays_inside_page() {
        let p = Params { gap: 3.0, ..Params::default() };
        let g = grid(&p);
        let (x, y) = slot(&p, &g, g.per_page() - 1);
        assert!(x + g.cw <= p.page_w + 0.01 && y + g.ch <= p.page_h + 0.01);
    }
}
