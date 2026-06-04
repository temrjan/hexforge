//! Visual theme ported from the design mockup (`docs/design-brief.md`).
//!
//! Warm near-black charcoal + amber accent, Inter (UI) / JetBrains Mono
//! (addresses). Provides the palette, font installation, the egui [`Style`],
//! and two shared painters (the hex mark and the highlighted-address layout).

use std::sync::Arc;

use eframe::egui::{
    self, text::LayoutJob, text::TextFormat, Color32, CornerRadius, FontData, FontDefinitions,
    FontFamily, FontId, Stroke, TextStyle,
};

/// Color tokens (1:1 with the mockup's CSS `:root`).
pub mod color {
    use eframe::egui::Color32;

    pub const WINDOW_BG: Color32 = Color32::from_rgb(0x14, 0x12, 0x0d);
    pub const CARD_BG: Color32 = Color32::from_rgb(0x1e, 0x1a, 0x13);
    pub const INPUT_BG: Color32 = Color32::from_rgb(0x10, 0x0e, 0x09);
    pub const INSET_BG: Color32 = Color32::from_rgb(0x10, 0x0d, 0x08);
    pub const SEC_BG: Color32 = Color32::from_rgb(0x22, 0x1e, 0x16);
    pub const SEC_HOVER: Color32 = Color32::from_rgb(0x2a, 0x25, 0x19);
    pub const SEC_PRESS: Color32 = Color32::from_rgb(0x1a, 0x16, 0x0f);
    pub const BORDER: Color32 = Color32::from_rgb(0x2e, 0x28, 0x1d);
    pub const BORDER_SOFT: Color32 = Color32::from_rgb(0x24, 0x1f, 0x17);
    pub const TXT: Color32 = Color32::from_rgb(0xec, 0xe6, 0xda);
    pub const TXT_MUTED: Color32 = Color32::from_rgb(0x96, 0x8f, 0x82);
    pub const TXT_FAINT: Color32 = Color32::from_rgb(0x6b, 0x65, 0x57);
    pub const ACCENT: Color32 = Color32::from_rgb(0xf5, 0xa6, 0x23);
    pub const ON_ACCENT: Color32 = Color32::from_rgb(0x1a, 0x15, 0x0b);
    pub const DANGER: Color32 = Color32::from_rgb(0xe8, 0x60, 0x4c);
    pub const SUCCESS: Color32 = Color32::from_rgb(0x62, 0xc0, 0x7e);
    pub const MONO_TXT: Color32 = Color32::from_rgb(0xd2, 0xcb, 0xbd);
}

/// Name of the SemiBold proportional family (titles / headers / buttons).
const STRONG: &str = "strong";

/// Installs fonts and the theme [`Style`] onto the context. Call once at start.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_global_style(build_style());
}

/// The SemiBold proportional family handle (for headings / button text).
pub fn strong_family() -> FontFamily {
    FontFamily::Name(STRONG.into())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-semibold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-SemiBold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains-mono".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMono-Regular.ttf"
        ))),
    );

    // Prepend our fonts; keep egui's defaults as fallback so Cyrillic gaps and
    // the 🔒 emoji still render.
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains-mono".to_owned());

    // SemiBold family = Inter-SemiBold then the proportional fallback chain.
    let mut strong = vec!["inter-semibold".to_owned()];
    strong.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts
        .families
        .insert(FontFamily::Name(STRONG.into()), strong);

    ctx.set_fonts(fonts);
}

fn build_style() -> egui::Style {
    egui::Style {
        visuals: build_visuals(),
        text_styles: [
            (TextStyle::Heading, FontId::new(18.0, strong_family())),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
            (TextStyle::Button, FontId::new(13.0, strong_family())),
            (
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            ),
        ]
        .into(),
        spacing: egui::style::Spacing {
            item_spacing: egui::vec2(8.0, 8.0),
            button_padding: egui::vec2(12.0, 7.0),
            interact_size: egui::vec2(40.0, 30.0),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn build_visuals() -> egui::Visuals {
    use color::{ACCENT, BORDER, BORDER_SOFT, INPUT_BG, SEC_BG, SEC_HOVER, SEC_PRESS, TXT};

    let r = CornerRadius::same(6);
    let mut v = egui::Visuals::dark();
    v.panel_fill = color::WINDOW_BG;
    v.window_fill = color::WINDOW_BG;
    v.extreme_bg_color = INPUT_BG; // TextEdit background
    v.faint_bg_color = color::CARD_BG;
    v.hyperlink_color = ACCENT;
    v.selection = egui::style::Selection {
        bg_fill: ACCENT.gamma_multiply(0.30),
        stroke: Stroke::new(1.0, ACCENT), // also tints the selected radio dot
    };

    let set = |w: &mut egui::style::WidgetVisuals, fill, stroke| {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.fg_stroke = Stroke::new(1.0, TXT);
        w.corner_radius = r;
        w.expansion = 0.0;
    };
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TXT);
    v.widgets.noninteractive.corner_radius = r;
    set(&mut v.widgets.inactive, SEC_BG, BORDER);
    set(&mut v.widgets.hovered, SEC_HOVER, BORDER);
    set(&mut v.widgets.active, SEC_PRESS, ACCENT); // focused input → amber border
    set(&mut v.widgets.open, SEC_HOVER, BORDER);
    v
}

/// Soft drop shadow for result cards (mockup: `0 2 8 rgba(0,0,0,.30)`).
pub fn card_shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(76),
    }
}

/// Paints the brand hex mark (pointy-top hexagon outline + `0x`).
pub fn paint_hexmark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    let c = rect.center();
    let radius = size * 0.46;
    let points: Vec<egui::Pos2> = (0..6_u8)
        .map(|i| {
            let a = std::f32::consts::FRAC_PI_2 + f32::from(i) * std::f32::consts::FRAC_PI_3;
            egui::pos2(c.x + radius * a.cos(), c.y - radius * a.sin())
        })
        .collect();
    painter.add(egui::Shape::closed_line(
        points,
        Stroke::new(1.4, color::ACCENT),
    ));
    painter.text(
        c,
        egui::Align2::CENTER_CENTER,
        "0x",
        FontId::new(size * 0.34, FontFamily::Monospace),
        color::ACCENT,
    );
}

/// Builds the address text with the matched word highlighted in the accent
/// color (first occurrence), the `0x` prefix faint, the rest in mono color.
pub fn address_layout(address: &str, word: &str) -> LayoutJob {
    let mono = FontId::new(13.0, FontFamily::Monospace);
    let fmt = |c: Color32| TextFormat {
        font_id: mono.clone(),
        color: c,
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    let hex = address.strip_prefix("0x").unwrap_or(address);
    job.append("0x", 0.0, fmt(color::TXT_FAINT));
    match hex.find(word) {
        Some(i) if !word.is_empty() => {
            job.append(&hex[..i], 0.0, fmt(color::MONO_TXT));
            job.append(&hex[i..i + word.len()], 0.0, fmt(color::ACCENT));
            job.append(&hex[i + word.len()..], 0.0, fmt(color::MONO_TXT));
        }
        _ => job.append(hex, 0.0, fmt(color::MONO_TXT)),
    }
    job
}
