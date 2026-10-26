# Token Print

Import finished token/card images (from Photoshop etc.), set how many copies of each,
and export a print-ready PDF whose page size equals your canvas (default 18 × 12 in).

- Import: "Import images…" or drag files onto the window (png / jpg / webp)
- Per-image copies, plus "Fill sheet" to fill the free slots on the current sheet
- Spacing: default 0.2 mm (buttons for 0.2 and 3 mm); edge margin optional
- Black corners toggle: paints the area outside the rounded corners black
  (also flattens transparency onto black when on, white when off)
- "Rotate cards 90° if more fit": e.g. at 3 mm spacing on 18×12 in this gives 20 per sheet instead of 18
- More cards than fit → extra sheets (pages) in the same PDF
- Images are stretched to the card size, so use 63×88 aspect images (or change the card size)

## Build
    cargo run --release
Linux needs GTK3 headers for the file dialog: `sudo apt install libgtk-3-dev`.
Core logic only (no GUI): `cargo test --lib --no-default-features`.
