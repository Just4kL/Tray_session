//! Движок раскладки главного окна (H1, шаг 2).
//!
//! Строит панели из `LayoutSpec`, контент рисуют замыкания вызывающего:
//! движок не знает про вкладки, кнопки и данные. К `update()` НЕ подключён
//! (шаг 3) — поведение UI не меняется.

use super::spec::LayoutSpec;

/// Контент зон главного окна. Вызывающий передаёт по одному замыканию
/// на зону; движок вызывает каждое ровно один раз внутри своей панели
/// (пропущенные при collapse зоны не вызываются).
#[allow(dead_code)] // TODO(H1-step-3): потребитель — update() в app.rs
pub struct WindowCallbacks<'a> {
    pub nav: Box<dyn FnMut(&mut egui::Ui) + 'a>,
    pub status: Box<dyn FnMut(&mut egui::Ui) + 'a>,
    pub center: Box<dyn FnMut(&mut egui::Ui) + 'a>,
}

/// Построить главное окно из спеки: nav слева, status снизу, center
/// с симметричными полями. Тонкий оркестратор поверх build_nav /
/// build_status / build_center — для инкрементальной интеграции
/// (шаг 3b+ подключает зоны по одной).
#[allow(dead_code)] // TODO(H1-step-3): потребитель — update() в app.rs
pub fn build_window(ctx: &egui::Context, spec: &LayoutSpec, mut cb: WindowCallbacks<'_>) {
    build_nav(ctx, spec, &mut cb.nav);
    build_status(ctx, spec, &mut cb.status);
    build_center(ctx, spec, &mut cb.center);
}

/// Левая панель навигации. Пропускается при collapse.
#[allow(dead_code)] // TODO(H1-step-3): потребитель — update() в app.rs
pub fn build_nav(
    ctx: &egui::Context,
    spec: &LayoutSpec,
    cb: &mut dyn FnMut(&mut egui::Ui),
) {
    let screen_w = ctx.screen_rect().width();
    let collapsed = spec
        .nav
        .collapse_below
        .is_some_and(|limit| screen_w < limit);
    if collapsed {
        return;
    }
    let sz = &spec.nav;
    let panel = egui::SidePanel::left("nav")
        .resizable(sz.resizable)
        .default_width(sz.default_w)
        .width_range(sz.min_w..=sz.max_w);
    let panel = if !sz.resizable && sz.default_w == sz.min_w && sz.default_w == sz.max_w {
        panel.exact_width(sz.default_w)
    } else {
        panel
    };
    panel.show(ctx, |ui| cb(ui));
}

/// Нижняя строка статуса. Пропускается при collapse.
#[allow(dead_code)] // TODO(H1-step-3): потребитель — update() в app.rs
pub fn build_status(
    ctx: &egui::Context,
    spec: &LayoutSpec,
    cb: &mut dyn FnMut(&mut egui::Ui),
) {
    let screen_w = ctx.screen_rect().width();
    let collapsed = spec
        .status
        .collapse_below
        .is_some_and(|limit| screen_w < limit);
    if collapsed {
        return;
    }
    let sz = &spec.status;
    egui::TopBottomPanel::bottom("status")
        .resizable(false)
        .exact_height(sz.min_h)
        .show(ctx, |ui| cb(ui));
}

/// Центральная область с симметричными полями страницы.
#[allow(dead_code)] // TODO(H1-step-3): потребитель — update() в app.rs
pub fn build_center(
    ctx: &egui::Context,
    spec: &LayoutSpec,
    cb: &mut dyn FnMut(&mut egui::Ui),
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let m = spec.page_margin.for_width(ui.available_width());
        let frame = egui::Frame::none().inner_margin(egui::Margin::symmetric(m, 0.0));
        frame.show(ui, |ui| cb(ui));
    });
}

#[cfg(test)]
mod tests {
    use super::super::presets::default_layout;
    use super::*;

    fn headless_ctx(w: f32, h: f32) -> egui::Context {
        let ctx = egui::Context::default();
        let rect =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(w, h));
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let _ = ctx.run(input, |_| {});
        ctx
    }

    fn noop_callbacks() -> WindowCallbacks<'static> {
        WindowCallbacks {
            nav: Box::new(|_| {}),
            status: Box::new(|_| {}),
            center: Box::new(|_| {}),
        }
    }

    fn run_window(ctx: &egui::Context, spec: &LayoutSpec) {
        let input = egui::RawInput::default();
        let _ = ctx.run(input, |ctx| {
            build_window(ctx, spec, noop_callbacks());
        });
    }

    #[test]
    fn build_window_runs_headless_without_panic() {
        // API движка работает без бэкенда: все три зоны построились,
        // паники нет. К update() не подключено.
        let ctx = headless_ctx(960.0, 640.0);
        run_window(&ctx, &default_layout());
    }

    #[test]
    fn build_window_skips_collapsed_zones() {
        // collapse_below выше ширины экрана: nav/status не строятся,
        // их замыкания не вызываются; center работает как раньше.
        use std::sync::{Arc, Mutex};
        let mut spec = default_layout();
        spec.nav.collapse_below = Some(5000.0);
        spec.status.collapse_below = Some(5000.0);
        let ctx = headless_ctx(960.0, 640.0);
        let nav_hit = Arc::new(Mutex::new(false));
        let status_hit = Arc::new(Mutex::new(false));
        let center_hit = Arc::new(Mutex::new(false));
        let (n, s, c) = (nav_hit.clone(), status_hit.clone(), center_hit.clone());
        let input = egui::RawInput::default();
        let _ = ctx.run(input, |ctx| {
            build_window(
                ctx,
                &spec,
                WindowCallbacks {
                    nav: Box::new(move |_| *n.lock().unwrap() = true),
                    status: Box::new(move |_| *s.lock().unwrap() = true),
                    center: Box::new(move |_| *c.lock().unwrap() = true),
                },
            );
        });
        assert!(!*nav_hit.lock().unwrap(), "схлопнутый nav вызван");
        assert!(!*status_hit.lock().unwrap(), "схлопнутый status вызван");
        assert!(*center_hit.lock().unwrap(), "center не вызван");
    }

    #[test]
    fn individual_builders_run_headless() {
        // Шаг 3a: зоны строятся и по отдельности — для инкрементального
        // подключения в update() (по одной панели за коммит).
        let ctx = headless_ctx(960.0, 640.0);
        let spec = default_layout();
        let input = egui::RawInput::default();
        let _ = ctx.run(input, |ctx| {
            build_nav(ctx, &spec, &mut |_| {});
            build_status(ctx, &spec, &mut |_| {});
            build_center(ctx, &spec, &mut |_| {});
        });
    }
}
