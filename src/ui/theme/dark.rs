//! Тёмный скин.
//!
//! Этап 1.1: заглушка, зеркало `default.rs`. Наполнение — коммит 1.4,
//! не сейчас.

use super::tokens::*;
use super::Skin;
use egui::Color32;

/// Тёмный скин.
#[allow(dead_code)] // TODO(phase-1.4): наполнение тёмной темы
pub struct DarkSkin {
    tokens: Tokens,
}

impl Default for DarkSkin {
    fn default() -> Self {
        Self {
            tokens: Tokens {
                spacing: Spacing {
                    xs: 4.0,
                    sm: 8.0,
                    md: 12.0,
                    lg: 16.0,
                    nav_w: 88.0,
                    status_h: 24.0,
                    row_h: 22.0,
                },
                radii: Radii {
                    widget: 4.0,
                    panel: 8.0,
                },
                typography: Typography {
                    body: 13.0,
                    caption: 11.0,
                    heading: 15.0,
                },
                palette: Palette {
                    surface: Color32::from_gray(24),
                    surface_alt: Color32::from_gray(36),
                    accent: Color32::from_rgb(80, 160, 240),
                    text: Color32::from_gray(235),
                    text_muted: Color32::from_gray(140),
                    border: Color32::from_gray(60),
                    ok: Color32::from_rgb(80, 200, 120),
                    warn: Color32::from_rgb(230, 180, 60),
                    err: Color32::from_rgb(230, 90, 90),
                },
            },
        }
    }
}

impl Skin for DarkSkin {
    fn id(&self) -> &'static str {
        "dark"
    }

    fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    fn egui_style(&self) -> egui::Style {
        // Маппинг токенов — в 1.4 вместе с наполнением.
        egui::Style::default()
    }
}
