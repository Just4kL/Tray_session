//! Скин по умолчанию.
//!
//! Этап 1.2.a.1: заполнен реальными значениями из рабочего
//! `steam_visuals()` и связанных функций (приложение B задания).
//! Значения копируются 1-в-1, без "улучшений". Чтение токенов —
//! с 1.2.a.2 (`steam_visuals()` -> `egui_style()`).

use super::tokens::*;
use super::Skin;
use egui::Color32;

/// Скин по умолчанию.
#[allow(dead_code)] // TODO(phase-1.2.a.2): поле читается при маппинге токенов в egui_style()
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
                    widget: 6.0,
                    panel: 8.0,
                },
                strokes: Strokes {
                    hairline: 1.0,
                    cursor: 2.0,
                },
                typography: Typography {
                    body: 13.0,
                    caption: 11.0,
                    heading: 15.0,
                },
                palette: Palette {
                    bg: BgPalette {
                        surface:        Color32::from_rgb(0x1B, 0x28, 0x38),
                        window:         Color32::from_rgb(0x17, 0x1D, 0x25),
                        extreme:        Color32::from_rgb(0x0E, 0x14, 0x1B),
                        faint:          Color32::from_rgb(0x22, 0x30, 0x3F),
                        sunken:         Color32::from_rgb(0x10, 0x18, 0x20),
                        noninteractive: Color32::from_rgb(0x16, 0x20, 0x2D),
                        raised:         Color32::from_rgb(0x2A, 0x2F, 0x35),
                    },
                    interactive: InteractivePalette {
                        inactive: InteractiveState {
                            bg:   Color32::from_rgb(0x2E, 0x4A, 0x62),
                            weak: Color32::from_rgb(0x2A, 0x40, 0x58),
                        },
                        hovered: InteractiveState {
                            bg:   Color32::from_rgb(0x3E, 0x6C, 0x8E),
                            weak: Color32::from_rgb(0x39, 0x5F, 0x80),
                        },
                        active: InteractiveState {
                            bg:   Color32::from_rgb(0x16, 0x32, 0x4A),
                            weak: Color32::from_rgb(0x11, 0x29, 0x3D),
                        },
                    },
                    accent: AccentPalette {
                        primary:   Color32::from_rgb(0x66, 0xC0, 0xF4),
                        selection: Color32::from_rgb(0x1B, 0x5F, 0x8A),
                        bright:    Color32::from_rgb(0x8E, 0xD4, 0xFF),
                    },
                    text: TextPalette {
                        primary:   Color32::from_rgb(0xC7, 0xD5, 0xE0),
                        muted:     Color32::from_rgb(0x9A, 0xA4, 0xAD),
                        on_accent: Color32::WHITE,
                    },
                    border: BorderPalette {
                        default:     Color32::from_rgb(0x3A, 0x42, 0x4C),
                        interactive: Color32::from_rgb(0x3A, 0x55, 0x6C),
                    },
                    semantic: SemanticPalette {
                        ok:        Color32::from_rgb(0x8C, 0xFF, 0x5A),
                        warn:      Color32::from_rgb(255, 165, 0),
                        warn_soft: Color32::from_rgb(0xFF, 0xC8, 0x3A),
                        err:       Color32::from_rgb(255, 0, 0),
                    },
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
        // Маппинг токенов — в 1.2.a.2. В 1.2.a.1 поведение не трогаем:
        // метод нигде не вызывается.
        egui::Style::default()
    }
}
