use eframe::egui::{
    self, load::SizedTexture, pos2, vec2, Color32, ColorImage, DragValue, Rect, Sense, Stroke,
    StrokeKind, TextureHandle, TextureOptions,
};
use image::DynamicImage;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
};
use tokenprint::{
    layout::{self, Grid, Params},
    render,
};

struct Item {
    name: String,
    src: DynamicImage,
    thumb: DynamicImage,
    qty: usize,
    tex: Option<(u64, TextureHandle)>,
}

struct App {
    items: Vec<Item>,
    canvas_in: [f32; 2],
    margin: f32,
    card: [f32; 2],
    gap: f32,
    auto_rotate: bool,
    black_corners: bool,
    radius: f32,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            items: vec![],
            canvas_in: [18.0, 12.0],
            margin: 0.0,
            card: [63.0, 88.0],
            gap: 0.2,
            auto_rotate: true,
            black_corners: true,
            radius: 3.2,
            status: String::new(),
        }
    }
}

impl App {
    fn params(&self) -> Params {
        Params {
            page_w: self.canvas_in[0] * layout::MM_PER_IN,
            page_h: self.canvas_in[1] * layout::MM_PER_IN,
            margin: self.margin,
            card_w: self.card[0],
            card_h: self.card[1],
            gap: self.gap,
            auto_rotate: self.auto_rotate,
        }
    }

    fn add_files(&mut self, paths: Vec<PathBuf>) {
        for p in paths {
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
                let res = std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| {
                    tokenprint::pdf_import::extract(&b, self.card[0], self.card[1], 0.04)
                });
                match res {
                    Ok(found) => {
                        self.status =
                            format!("Imported {} card(s) from {}", found.len(), p.display());
                        for (i, f) in found.into_iter().enumerate() {
                            let thumb = f.img.thumbnail(480, 680);
                            self.items.push(Item {
                                name: format!("{stem} {}", i + 1),
                                src: f.img,
                                thumb,
                                qty: f.copies,
                                tex: None,
                            });
                        }
                    }
                    Err(e) => self.status = format!("PDF import failed ({}): {e}", p.display()),
                }
                continue;
            }

            match image::open(&p) {
                Ok(img) => {
                    let name = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let thumb = img.thumbnail(480, 680);
                    self.items.push(Item {
                        name,
                        src: img,
                        thumb,
                        qty: 1,
                        tex: None,
                    });
                }
                Err(e) => self.status = format!("Could not open {}: {e}", p.display()),
            }
        }
    }

    /// Rebuild preview textures whenever a setting that changes the pixels changes.
    fn ensure_textures(&mut self, ctx: &egui::Context, rotated: bool) {
        let mut h = DefaultHasher::new();
        (
            self.black_corners,
            self.radius.to_bits(),
            self.card[0].to_bits(),
            self.card[1].to_bits(),
            rotated,
        )
            .hash(&mut h);
        let key = h.finish();
        for it in &mut self.items {
            if it.tex.as_ref().map(|t| t.0) != Some(key) {
                let rgb = render::prepare(
                    &it.thumb,
                    self.black_corners,
                    self.radius,
                    self.card[0],
                    self.card[1],
                    rotated,
                );
                let ci = ColorImage::from_rgb(
                    [rgb.width() as usize, rgb.height() as usize],
                    rgb.as_raw(),
                );
                it.tex = Some((key, ctx.load_texture(&it.name, ci, TextureOptions::LINEAR)));
            }
        }
    }

    fn export(&mut self, p: &Params, g: &Grid) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name("tokens.pdf")
            .save_file()
        else {
            return;
        };
        let imgs: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.qty > 0)
            .map(|i| {
                (
                    render::prepare(
                        &i.src,
                        self.black_corners,
                        self.radius,
                        self.card[0],
                        self.card[1],
                        g.rotated,
                    ),
                    i.qty,
                )
            })
            .collect();
        self.status = match render::export_pdf(p, g, &imgs)
            .and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()))
        {
            Ok(_) => format!("Saved {}", path.display()),
            Err(e) => format!("Export failed: {e}"),
        };
    }

    /// Renders the side menu.
    /// Top part: import button + scrollable item list (fills the remaining height).
    fn items_ui(&mut self, ui: &mut egui::Ui, g: &Grid) {
        ui.heading("Token Print");
        if ui.button("Import images…").clicked() {
            let pick_files = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp", "pdf"])
                .pick_files();

            if let Some(f) = pick_files {
                self.add_files(f);
            }
        }
        ui.weak("or drop image files onto this window");
        ui.separator();

        let per = g.per_page();
        let total: usize = self.items.iter().map(|i| i.qty).sum();
        let mut remove = None;

        egui::ScrollArea::vertical()
            .id_salt("item_list")
            .auto_shrink([false, false]) // always fill the space left by the bottom panel
            .show(ui, |ui| {
                for idx in 0..self.items.len() {
                    let others = total - self.items[idx].qty;
                    let it = &mut self.items[idx];
                    ui.horizontal(|ui| {
                        if let Some((_, t)) = &it.tex {
                            ui.add(
                                egui::Image::new(SizedTexture::new(t.id(), t.size_vec2()))
                                    .max_height(48.0),
                            );
                        }
                        ui.vertical(|ui| {
                            ui.label(&it.name);
                            ui.horizontal(|ui| {
                                ui.add(DragValue::new(&mut it.qty).range(0..=999).suffix("×"));
                                if ui
                                    .small_button("Fill sheet")
                                    .on_hover_text("Fill the free slots on the current sheet")
                                    .clicked()
                                    && per > 0
                                {
                                    it.qty = per - others % per;
                                }
                                if ui.small_button("X").clicked() {
                                    remove = Some(idx);
                                }
                            });
                        });
                    });
                }
            });

        if let Some(i) = remove {
            self.items.remove(i);
        }
    }

    /// Bottom part: canvas/card settings, summary and export. Always visible.
    fn settings_ui(&mut self, ui: &mut egui::Ui, p: &Params, g: &Grid) {
        let per = g.per_page();
        let total: usize = self.items.iter().map(|i| i.qty).sum();

        ui.label("Canvas");
        ui.horizontal(|ui| {
            ui.label("Size (in)");
            ui.add(
                DragValue::new(&mut self.canvas_in[0])
                    .range(2.0..=100.0)
                    .speed(0.1),
            );
            ui.label("×");
            ui.add(
                DragValue::new(&mut self.canvas_in[1])
                    .range(2.0..=100.0)
                    .speed(0.1),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Edge margin (mm)");
            ui.add(
                DragValue::new(&mut self.margin)
                    .range(0.0..=50.0)
                    .speed(0.1),
            );
        });
        ui.separator();
        ui.label("Cards");
        ui.horizontal(|ui| {
            ui.label("Size (mm)");
            ui.add(
                DragValue::new(&mut self.card[0])
                    .range(10.0..=300.0)
                    .speed(0.1),
            );
            ui.label("×");
            ui.add(
                DragValue::new(&mut self.card[1])
                    .range(10.0..=300.0)
                    .speed(0.1),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Spacing (mm)");
            ui.add(
                DragValue::new(&mut self.gap)
                    .range(0.0..=20.0)
                    .speed(0.05)
                    .fixed_decimals(2),
            );
            if ui.small_button("0.2").clicked() {
                self.gap = 0.2;
            }
            if ui.small_button("3").clicked() {
                self.gap = 3.0;
            }
        });
        ui.checkbox(&mut self.auto_rotate, "Rotate cards 90° if more fit");
        ui.checkbox(&mut self.black_corners, "Black corners");
        ui.add_enabled_ui(self.black_corners, |ui| {
            ui.horizontal(|ui| {
                ui.label("Corner radius (mm)");
                ui.add(
                    DragValue::new(&mut self.radius)
                        .range(0.0..=10.0)
                        .speed(0.05)
                        .fixed_decimals(2),
                );
            });
        });
        ui.separator();

        let pages = if per == 0 { 0 } else { total.div_ceil(per) };
        let label_rotated = if g.rotated { ", rotated" } else { "" };
        ui.label(format!(
            "{per} per sheet ({}×{}{}) · {total} cards · {pages} sheet(s)",
            g.cols, g.rows, label_rotated
        ));

        let is_enabled = total > 0 && per > 0;
        if ui
            .add_enabled(is_enabled, egui::Button::new("Export PDF…"))
            .clicked()
        {
            self.export(p, g);
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
    }

    fn preview(&self, ui: &mut egui::Ui, p: &Params, g: &Grid) {
        let per = g.per_page();
        if per == 0 {
            ui.label("Cards do not fit on this canvas.");
            return;
        }
        let cards: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .flat_map(|(i, it)| std::iter::repeat(i).take(it.qty))
            .collect();
        let pages = cards.len().div_ceil(per).max(1);
        let scale = ui.available_width().min(1000.0) / p.page_w;
        for pg in 0..pages {
            let (rect, _) =
                ui.allocate_exact_size(vec2(p.page_w * scale, p.page_h * scale), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, Color32::WHITE);
            for s in 0..per {
                let (x, y) = layout::slot(p, g, s);
                let r =
                    Rect::from_min_size(rect.min + vec2(x, y) * scale, vec2(g.cw, g.ch) * scale);
                match cards
                    .get(pg * per + s)
                    .and_then(|&i| self.items[i].tex.as_ref())
                {
                    Some((_, t)) => {
                        painter.image(
                            t.id(),
                            r,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    None => {
                        painter.rect_stroke(
                            r,
                            0.0,
                            Stroke::new(0.5_f32, Color32::LIGHT_GRAY),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
            ui.add_space(12.0);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });

        if !dropped.is_empty() {
            self.add_files(dropped);
        }

        let p = self.params();
        let g = layout::grid(&p);
        self.ensure_textures(ctx, g.rotated);

        egui::SidePanel::left("side")
            .min_width(330.0)
            .show(ctx, |ui| {
                egui::TopBottomPanel::bottom("side_settings")
                    .resizable(false)
                    .show_inside(ui, |ui| self.settings_ui(ui, &p, &g));

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(ui, |ui| self.items_ui(ui, &g));
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| self.preview(ui, &p, &g));
        });
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Token Print",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}
