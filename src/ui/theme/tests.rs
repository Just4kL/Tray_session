//! Golden-тест steam-темы.
//!
//! Эталон — замороженная копия `steam_visuals()` из `121cd12:src/app.rs`
//! (функция удалена в 1.2.a.2). Если тест падает —
//! `DefaultSkin::egui_style()` разошёлся с историческим визуалом.
//! Обновление эталона ДОПУСТИМО только при осознанной смене темы
//! (например, добавление dark-скина в 1.4); в этом случае в
//! коммит-мессадже явно указать, какие поля `Visuals` изменились.

use super::default::DefaultSkin;
use super::Skin;

/// Эталон: копия `steam_visuals()` из 121cd12, построчно, без "по смыслу".
fn reference_visuals() -> egui::Visuals {
    use egui::Color32 as C;
    let mut v = egui::Visuals::dark();
    v.panel_fill = C::from_rgb(0x1B, 0x28, 0x38);
    v.window_fill = C::from_rgb(0x17, 0x1D, 0x25);
    v.extreme_bg_color = C::from_rgb(0x0E, 0x14, 0x1B);
    v.faint_bg_color = C::from_rgb(0x22, 0x30, 0x3F);
    v.code_bg_color = C::from_rgb(0x10, 0x18, 0x20);
    v.text_cursor.stroke = egui::Stroke::new(2.0_f32, C::from_rgb(0x66, 0xC0, 0xF4));
    v.hyperlink_color = C::from_rgb(0x66, 0xC0, 0xF4);
    v.selection.bg_fill = C::from_rgb(0x1B, 0x5F, 0x8A);
    v.selection.stroke = egui::Stroke::new(1.0_f32, C::WHITE);
    v.widgets.noninteractive.bg_fill = C::from_rgb(0x16, 0x20, 0x2D);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, C::from_rgb(0xC7, 0xD5, 0xE0));
    v.widgets.inactive.bg_fill = C::from_rgb(0x2E, 0x4A, 0x62);
    v.widgets.inactive.weak_bg_fill = C::from_rgb(0x2A, 0x40, 0x58);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, C::WHITE);
    v.widgets.inactive.rounding = egui::Rounding::same(6.0);
    v.widgets.hovered.bg_fill = C::from_rgb(0x3E, 0x6C, 0x8E);
    v.widgets.hovered.weak_bg_fill = C::from_rgb(0x39, 0x5F, 0x80);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, C::WHITE);
    v.widgets.hovered.rounding = egui::Rounding::same(6.0);
    v.widgets.active.bg_fill = C::from_rgb(0x16, 0x32, 0x4A);
    v.widgets.active.weak_bg_fill = C::from_rgb(0x11, 0x29, 0x3D);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, C::WHITE);
    v.widgets.active.rounding = egui::Rounding::same(6.0);
    v.widgets.open.bg_fill = C::from_rgb(0x22, 0x30, 0x3F);
    v.widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, C::from_rgb(0x66, 0xC0, 0xF4));
    v.widgets.open.rounding = egui::Rounding::same(6.0);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, C::from_rgb(0x3A, 0x55, 0x6C));
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, C::from_rgb(0x66, 0xC0, 0xF4));
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, C::from_rgb(0x8E, 0xD4, 0xFF));
    v
}

#[test]
fn default_skin_visuals_match_reference() {
    let actual = DefaultSkin::default().egui_style().visuals;
    let expected = reference_visuals();
    assert_eq!(format!("{:?}", actual), format!("{:?}", expected));
}
