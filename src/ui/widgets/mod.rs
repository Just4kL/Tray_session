//! Переиспользуемые виджеты.
//!
//! Этап 3.0: заглушка. Сюда переедут повторяющиеся куски из `app.rs`
//! (карточки разделов, кнопки в стиле Material, строки таблицы) по мере
//! выноса вью в фазе 3.

use egui::{Color32, Response, Sense, Stroke, TextStyle, Ui, Vec2};

/// Кружок-индикатор состояния: «идёт запись», «проверка GPU».
///
/// Рисуется ВЕКТОРОМ, а не символом. Причина конкретная: `●` U+25CF и
/// `◌` U+25CC есть только в шрифте Hack, а Hack входит лишь в
/// `FontFamily::Monospace`. Пропорциональное семейство egui — это
/// Ubuntu-Light → NotoEmoji-Regular → emoji-icon-font, и ни в одном из
/// них этих глифов нет. Поэтому в подписях вместо кружка рисовался
/// пустой квадрат (tofu). Вектор от шрифтов не зависит вообще.
///
/// `filled` — сплошной кружок (идёт запись) или контурный (ждём
/// подтверждения нагрузки). Цвет задаёт вызывающий: он семантический
/// (`palette.semantic.recording` / `.attention`), а не декоративный.
pub fn status_dot(ui: &mut Ui, color: Color32, filled: bool) -> Response {
    // Кружок по высоте строки основного текста: рядом с ним обычно стоит
    // подпись тем же стилем, и они должны совпадать по вертикали.
    let h = ui.text_style_height(&TextStyle::Body);
    let d = (h * 0.45).clamp(5.0, 10.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(d, h), Sense::hover());
    let center = rect.center();
    let r = d * 0.5;
    if filled {
        ui.painter().circle_filled(center, r, color);
    } else {
        ui.painter().circle_stroke(center, r, Stroke::new(1.5_f32, color));
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Один headless-кадр: возвращает нарисованные фигуры.
    fn shapes_of(f: impl FnOnce(&mut Ui)) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(200.0, 60.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| f(ui));
            },
        );
        out.shapes.into_iter().map(|c| c.shape).collect()
    }

    fn only_circle(shapes: &[egui::Shape]) -> &egui::epaint::CircleShape {
        shapes
            .iter()
            .find_map(|s| match s {
                egui::Shape::Circle(c) => Some(c),
                _ => None,
            })
            .expect("кружок не нарисован")
    }

    #[test]
    fn status_dot_filled_uses_the_semantic_color() {
        let color = egui::Color32::from_rgb(0x4C, 0xAF, 0x50);
        let shapes = shapes_of(|ui| {
            status_dot(ui, color, true);
        });
        let c = only_circle(&shapes);
        assert_eq!(c.fill, color, "сплошной кружок залит цветом вызывающего");
        assert!(
            (2.5..=5.0).contains(&c.radius),
            "радиус {} вне разумных границ",
            c.radius
        );
    }

    #[test]
    fn status_dot_outline_has_no_fill() {
        let color = egui::Color32::from_rgb(0xE0, 0xA0, 0x30);
        let shapes = shapes_of(|ui| {
            status_dot(ui, color, false);
        });
        let c = only_circle(&shapes);
        assert_eq!(
            c.fill,
            egui::Color32::TRANSPARENT,
            "контурный кружок не заливается"
        );
        assert_eq!(c.stroke.color, color);
        assert!(c.stroke.width > 0.0, "у контура должна быть толщина");
    }

    #[test]
    fn status_dot_does_not_draw_a_glyph() {
        // Страховка от возврата к символу: в фигурах не должно быть
        // текста — только круг. Если кто-то снова напишет "●", тут
        // появится Shape::Text.
        let shapes = shapes_of(|ui| {
            status_dot(ui, egui::Color32::WHITE, true);
        });
        assert!(
            !shapes.iter().any(|s| matches!(s, egui::Shape::Text(_))),
            "индикатор обязан оставаться вектором, а не глифом"
        );
    }
}
