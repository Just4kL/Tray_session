//! Скин по умолчанию.
//!
//! Этап 1.1: заглушка — доказывает, что структура компилируется.
//! Значения произвольные, лишь бы типы совпадали. Наполнение реальными
//! цветами из аудита — коммит 1.3.

use super::tokens::*;
use super::Skin;
use egui::Color32;

/// Скин по умолчанию.
#[allow(dead_code)] // TODO(phase-1.3): поле читается при маппинге токенов в egui_style()
pub struct DefaultSkin {
    tokens: Tokens,
}

impl Default for DefaultSkin {
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
                    surface: Color32::from_gray(32),
                    surface_alt: Color32::from_gray(48),
                    accent: Color32::from_rgb(80, 160, 240),
                    text: Color32::from_gray(230),
                    text_muted: Color32::from_gray(150),
                    border: Color32::from_gray(70),
                    ok: Color32::from_rgb(80, 200, 120),
                    warn: Color32::from_rgb(230, 180, 60),
                    err: Color32::from_rgb(230, 90, 90),
                },
            },
        }
    }
}

impl Skin for DefaultSkin {
    fn id(&self) -> &'static str {
        "default"
    }

    fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    fn egui_style(&self) -> egui::Style {
        // Маппинг токенов — в 1.3. В 1.1 поведение не трогаем:
        // метод нигде не вызывается.
        egui::Style::default()
    }
}
