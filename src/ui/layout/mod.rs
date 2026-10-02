//! Каркас раскладки.
//!
//! Этап 3.0: заглушка. Фаза 2 положит сюда `spec.rs` (описание зон),
//! `presets.rs` (пресет по умолчанию: навигация 88pt, статус 24pt) и
//! `engine.rs` — единственное место в проекте, где создаются панели
//! `SidePanel`/`TopBottomPanel`/`CentralPanel`.

pub mod engine;
pub mod presets;
pub mod spec;
// engine.rs — шаг 2 H1: build_window() поверх spec + presets.
