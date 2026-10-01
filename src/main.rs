#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Скрывает консольное окно в release. В debug оставлено для отладки.
// Причина: .exe без этого атрибута — консольное приложение. При
// двойном клике Windows открывает консоль, потом программу. Программа
// скрывается в трей, консоль остаётся.
mod app;
mod app_state;
mod config;
mod db;
mod detector;
mod export;
mod gpu;
mod log;
mod monitor;
mod shortcuts;
mod sound;
mod testpaths;
mod ui;
mod update;
mod uninstall;

use app::{initial_games, TrackerApp, VERSION};
use app_state::{AppCmd, TrayCmd};
use config::AppConfig;
use std::sync::{mpsc, Arc, RwLock};

/// Иконка программы: скруглённый квадрат в акцентном цвете темы Steam
/// (#66C0F4) с белым «треугольникомplay» по центру — узнаётся и в трее
/// (16–32 px), и в заголовке окна.
///
/// Раньше в трее рисовался безликий зелёный квадрат, а у окна стояла
/// системная иконка по умолчанию — программа выглядела «не своей».
/// Теперь это одна и та же картинка в обоих местах.
pub fn build_app_icon(size: u32) -> Vec<u8> {
    let w = size;
    let h = size;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    // Радиус скругления ~22% от стороны.
    let r = (w as f32) * 0.22;
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    // Треугольник (play) в центре.
    let tri_h = h as f32 * 0.34;
    let tri_w = tri_h * 0.86;
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            // Скругление: вне радиуса углов — прозрачно.
            let in_corner = (cx - fx).abs() > cx - r && (cy - fy).abs() > cy - r;
            let outside = if in_corner {
                let qx = (cx - fx).abs() - (cx - r);
                let qy = (cy - fy).abs() - (cy - r);
                qx.max(0.0).powi(2) + qy.max(0.0).powi(2) > r * r
            } else {
                false
            };
            if outside {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // Треугольник: вершины (cx - tri_w/2, cy - tri_h/2),
            // (cx - tri_w/2, cy + tri_h/2), (cx + tri_w/2, cy).
            let x0 = cx - tri_w / 2.0;
            let x1 = cx + tri_w / 2.0;
            let y0 = cy - tri_h / 2.0;
            let y1 = cy + tri_h / 2.0;
            let inside_tri = {
                // Точка внутри треугольника, если знаки векторных
                // произведений по всем трём рёбрам совпадают. Проверка
                // знаков, а не «>= 0», — чтобы не зависеть от порядка
                // обхода вершин (он инвертирован, т.к. y растёт вниз).
                let cross = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| {
                    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
                };
                let p = (fx, fy);
                let d1 = cross((x0, y0), (x0, y1), p);
                let d2 = cross((x0, y1), (x1, cy), p);
                let d3 = cross((x1, cy), (x0, y0), p);
                let s = |v: f32| if v > 0.0 {
                    1
                } else if v < 0.0 {
                    -1
                } else {
                    0
                };
                s(d1) == s(d2) && s(d2) == s(d3) && s(d1) != 0
            };
            if inside_tri {
                rgba.extend_from_slice(&[0xFF, 0xFF, 0xFF, 255]);
            } else {
                rgba.extend_from_slice(&[0x66, 0xC0, 0xF4, 255]);
            }
        }
    }
    rgba
}

/// Показать главное окно через WinAPI, минуя event loop eframe.
/// Нужно для трей-меню: при Minimized(true) eframe спит, AppCmd::Show
/// не обрабатывается. Прямой ShowWindow разбудит окно.
#[cfg(windows)]
fn show_main_window_native() {
    use winapi::um::winuser::{
        FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };
    let title: Vec<u16> = "Tray Session\0".encode_utf16().collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return;
        }
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
        SetForegroundWindow(hwnd);
    }
}

#[cfg(not(windows))]
fn show_main_window_native() {}

fn spawn_tray(rx_tooltip: mpsc::Receiver<TrayCmd>, tx_app: mpsc::Sender<AppCmd>) {
    // ВАЖНО (Windows): tray-icon требует прокачку Win32-сообщений на том потоке,
    // где создана иконка, иначе тултип/меню/клики мертвы. Отдельный winit
    // EventLoop здесь невозможен (один на процесс — уже занят eframe),
    // поэтому крутим классический PeekMessage-памп + опрос каналов.
    std::thread::spawn(move || {
        use tray_icon::menu::{Menu, MenuEvent, MenuItem};
        use tray_icon::{MouseButton, TrayIconBuilder, TrayIconEvent};

        let menu = Menu::new();
        let open_item = MenuItem::new("Открыть окно", true, None);
        let quit_item = MenuItem::new("Выход", true, None);
        let _ = menu.append(&open_item);
        let _ = menu.append(&quit_item);

        // Иконка та же, что и у окна: иначе в трее и в заголовке были
        // разные картинки (в трее — просто квадрат, в окне — системная).
        let rgba = build_app_icon(32);
        let icon = tray_icon::Icon::from_rgba(rgba, 32, 32).expect("icon");
        let _tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Tray Session")
            .with_icon(icon)
            .build()
            .expect("tray");

        let menu_rx = MenuEvent::receiver();
        let tray_rx = TrayIconEvent::receiver();

        // Храним текущий тултип, чтобы не дёргать API без нужды
        let mut current_tt = String::new();
        // Активные глобальные хоткеи секундомера (uid -> действие).
        let mut hotkeys: Vec<crate::shortcuts::HotkeyReg> = Vec::new();
        loop {
            for fired in pump_win32_messages() {
                // WM_HOTKEY: проверяем сторону правого Shift при нужде
                let reg = hotkeys.iter().find(|r| r.uid as usize == fired);
                let fire = match reg {
                    Some(r) if r.rshift_only => crate::shortcuts::rshift_down(),
                    Some(_) => true,
                    None => false,
                };
                if !fire {
                    continue;
                }
                let cmd = match reg.map(|r| r.action.as_str()) {
                    Some("stopwatch_toggle") => AppCmd::StopwatchToggle,
                    Some("stopwatch_lap") => AppCmd::StopwatchLap,
                    Some("stopwatch_stop") => AppCmd::StopwatchStop,
                    Some("stopwatch_pin") => AppCmd::StopwatchPin,
                    Some("opacity_up") => AppCmd::OpacityDelta(5),
                    Some("opacity_down") => AppCmd::OpacityDelta(-5),
                    Some("strip_opacity_up") => AppCmd::StripOpacity(5),
                    Some("strip_opacity_down") => AppCmd::StripOpacity(-5),
                    Some("strip_pos_prev") => AppCmd::StripStep(-1),
                    Some("strip_pos_next") => AppCmd::StripStep(1),
                    _ => continue,
                };
                let _ = tx_app.send(cmd);
            }
            // Команды тултипа (неблокирующе, пачка)
            while let Ok(cmd) = rx_tooltip.try_recv() {
                match cmd {
                    TrayCmd::Tooltip(t) => {
                        if t != current_tt {
                            current_tt = t.clone();
                            _tray.set_tooltip(Some(t)).ok();
                        }
                    }
                    TrayCmd::StopwatchHotkeys(regs) => {
                        apply_global_hotkeys(&mut hotkeys, regs);
                    }
                }
            }
            if let Ok(ev) = menu_rx.try_recv() {
                if ev.id == open_item.id() {
                    // WinAPI напрямую — eframe спит при Minimized(true).
                    show_main_window_native();
                    // AppCmd::Show — на случай, если eframe всё-таки проснётся и
                    // захочет синхронизировать своё внутреннее состояние viewport.
                    let _ = tx_app.send(AppCmd::Show);
                } else if ev.id == quit_item.id() {
                    let _ = tx_app.send(AppCmd::Quit);
                    // Даём главному потоку закрыться
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    std::process::exit(0);
                }
            }
            if let Ok(ev) = tray_rx.try_recv() {
                match ev {
                    TrayIconEvent::Click { button, .. } => {
                        if button == MouseButton::Left {
                            show_main_window_native();
                            let _ = tx_app.send(AppCmd::Show);
                        }
                    }
                    TrayIconEvent::DoubleClick { .. } => {
                        show_main_window_native();
                        let _ = tx_app.send(AppCmd::Show);
                    }
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    });
}

/// Одна итерация Win32 message pump для потока трея.
/// Без неё скрытое окно tray-icon не получает колбэки иконки/меню.
/// Возвращает id сработавших глобальных хоткеев (WM_HOTKEY).
#[cfg(windows)]
fn pump_win32_messages() -> Vec<usize> {
    use winapi::um::winuser::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_HOTKEY,
    };
    let mut fired = Vec::new();
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            if msg.message == WM_HOTKEY {
                fired.push(msg.wParam);
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    fired
}

#[cfg(not(windows))]
fn pump_win32_messages() -> Vec<usize> {
    Vec::new()
}

/// Перерегистрация глобальных хоткеев секундомера (поток трея).
#[cfg(windows)]
fn apply_global_hotkeys(
    current: &mut Vec<crate::shortcuts::HotkeyReg>,
    regs: Vec<crate::shortcuts::HotkeyReg>,
) {
    use winapi::um::winuser::{RegisterHotKey, UnregisterHotKey};
    unsafe {
        for r in current.iter() {
            UnregisterHotKey(std::ptr::null_mut(), r.uid as i32);
        }
    }
    *current = regs;
    unsafe {
        for r in current.iter() {
            // Ошибка (комбинация занята) — молча пропускаем, шорткат
            // переназначается во вкладке «Управление».
            RegisterHotKey(std::ptr::null_mut(), r.uid as i32, r.mods, r.vk);
        }
    }
}

#[cfg(not(windows))]
fn apply_global_hotkeys(
    current: &mut Vec<crate::shortcuts::HotkeyReg>,
    regs: Vec<crate::shortcuts::HotkeyReg>,
) {
    *current = regs;
}

/// Завершить старый main-процесс по PID из main_pid.txt (ветка --updated).
/// Best effort: файла/процесса уже нет — значит, всё хорошо, выходим молча.
#[cfg(windows)]
fn kill_old_main_by_pid_file() {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
    use winapi::um::winnt::PROCESS_TERMINATE;
    let pid: u32 = match std::fs::read_to_string(
        crate::update::tmp_dir(&crate::update::program_dir()).join(crate::update::MAIN_PID_FILE),
    )
    .ok()
    .and_then(|s| s.trim().parse().ok())
    {
        Some(p) => p,
        None => return,
    };
    if pid == std::process::id() {
        return;
    }
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if h.is_null() {
            return;
        }
        TerminateProcess(h, 0);
        CloseHandle(h);
    }
    // Лога тут нет: log::init ещё не вызван, запись ушла бы в никуда.
}

#[cfg(test)]
mod tests {
    use super::build_app_icon;
    use super::SINGLE_INSTANCE_MUTEX;

    /// Имя mutex-а single-instance не должно меняться молча: иначе после
    /// обновления живые процессы со старым именем не будут находиться
    /// новыми, и защита от второго экземпляра отключится.
    #[cfg(windows)]
    #[test]
    fn single_instance_mutex_name_is_stable() {
        assert_eq!(
            SINGLE_INSTANCE_MUTEX, "Local\\TraySession_SingleInstance",
            "имя mutex-а изменилось — живые процессы его не найдут"
        );
    }

    /// Иконка должна быть непрозрачной по центру и прозрачной по углам
    /// (скругление) — иначе в трее будет белый/чёрный квадрат.
    #[test]
    fn app_icon_shape() {
        for size in [16u32, 32, 64] {
            let px = build_app_icon(size);
            assert_eq!(px.len(), (size * size * 4) as usize, "размер {size}: неверная длина");
            let at = |x: u32, y: u32| -> [u8; 4] {
                let i = ((y * size + x) * 4) as usize;
                [px[i], px[i + 1], px[i + 2], px[i + 3]]
            };
            // Угол прозрачный.
            assert_eq!(at(0, 0)[3], 0, "{size}: угол должен быть прозрачным");
            // Центр непрозрачный.
            let c = at(size / 2, size / 2);
            assert_eq!(c[3], 255, "{size}: центр должен быть непрозрачным");
            // Центр белый (треугольник play).
            assert_eq!(&c[..3], &[255, 255, 255], "{size}: в центре должен быть play-треугольник");
            // Фон — акцент темы.
            let left_mid = at((size / 8).max(1), size / 2);
            assert_eq!(
                &left_mid[..3],
                &[0x66, 0xC0, 0xF4],
                "{size}: фон должен быть акцентным #66C0F4"
            );
        }
    }

    /// У иконки должен быть хотя бы один полностью прозрачный пиксель
    /// (иначе это квадрат) и хотя бы один непрозрачный (иначе пусто).
    #[test]
    fn app_icon_has_alpha_and_opaque() {
        let px = build_app_icon(32);
        let transparent = px.chunks_exact(4).filter(|p| p[3] == 0).count();
        let opaque = px.chunks_exact(4).filter(|p| p[3] == 255).count();
        assert!(transparent > 0, "нет прозрачных пикселей — иконка квадратная");
        assert!(opaque > 0, "нет непрозрачных пикселей — иконка пустая");
        assert!(
            transparent < opaque,
            "прозрачной должно быть меньше, чем непрозрачной"
        );
    }
}

/// Имя именованного мьютекса single-instance.
///
/// Вынесено в константу и зафиксировано тестом: если имя поменять, живые
/// процессы со старым именем перестанут находиться новыми — и защита от
/// второго экземпляра молча отключится после обновления.
#[cfg(windows)]
const SINGLE_INSTANCE_MUTEX: &str = "Local\\TraySession_SingleInstance";

fn main() -> eframe::Result {
    // Фоновый процесс обновления. Это тот же самый .exe, запущенный с
    // флагом --updater: отдельный процесс нужен, чтобы (а) интерфейс не
    // висел на загрузке и (б) заменять .exe мог только тот, кто его не
    // держит открытым. Окно при этом не создаётся.
    let argv: Vec<String> = std::env::args().collect();
    if let Some(upd) = update::parse_args(&argv) {
        std::process::exit(update::run_updater(upd.parent_pid, upd.wait_secs, &upd.channel));
    }
    // Перезапуск после обновления (R4): updater уже заменил файлы и
    // запустил нас с --updated. Старого процесса уже нет (updater ждал
    // его смерти), но на всякий случай ветка ниже умеет его завершить.

    // Режим деактиватора. Отдельного маленького бинарника не делаем
    // намеренно: он тянул бы за собой копию логирования, работы с базой и
    // выгрузки CSV, а они должны быть ровно одни и те же, что и у программы.
    // Вместо этого один и тот же файл копируется как `_uninstall.exe` и сам
    // узнаёт себя по имени.
    if uninstall::is_uninstaller() {
        crate::log::init(&update::program_dir());
        crate::log::info("запуск в режиме деактивации");
        std::process::exit(uninstall::run(&argv));
    }

    // Single-instance: не даём запуститься второму экземпляру.
    // Оверлеи (секундомер, полоска) имеют одинаковый title во всех
    // процессах — FindWindowW в ensure_thickframe/apply_overlay_opacity
    // без этого попадает в чужой HWND, и позиция/прозрачность идут
    // в случайное окно. Баг C5.
    //
    // Режимы --updater и --uninstaller вышли выше и mutex не берут —
    // это короткоживущие вспомогательные процессы, они должны
    // запускаться параллельно основной программе.
    //
    // Окно ищем по точному title "Tray Session" (см. run_native ниже).
    // Если оно скрыто в трее (Visible(false)), HWND всё равно существует,
    // ShowWindow(SW_SHOW) + SetForegroundWindow его показывают.
    //
    // R4: запуск с --updated после замены файлов. Старого процесса быть
    // не должно (updater ждал смерти), но если mutex всё ещё занят —
    // завершаем владельца по PID из main_pid.txt, ждём 2 с и захватываем
    // mutex заново. Без флага — старое поведение (показать окно + exit).
    #[cfg(windows)]
    let _single_instance_mutex = {
        use winapi::shared::winerror::ERROR_ALREADY_EXISTS;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::synchapi::CreateMutexW;
        use winapi::um::winuser::{
            FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
        };
        let updated_mode = argv.iter().any(|a| a == update::UPDATED_FLAG);
        let mutex_name: Vec<u16> = SINGLE_INSTANCE_MUTEX
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let mut h = CreateMutexW(std::ptr::null_mut(), 0, mutex_name.as_ptr());
            if GetLastError() == ERROR_ALREADY_EXISTS && updated_mode {
                kill_old_main_by_pid_file();
                std::thread::sleep(std::time::Duration::from_secs(2));
                h = CreateMutexW(std::ptr::null_mut(), 0, mutex_name.as_ptr());
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let title: Vec<u16> = "Tray Session\0".encode_utf16().collect();
                let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
                if !hwnd.is_null() {
                    if IsIconic(hwnd) != 0 {
                        ShowWindow(hwnd, SW_RESTORE);
                    } else {
                        ShowWindow(hwnd, SW_SHOW);
                    }
                    SetForegroundWindow(hwnd);
                }
                std::process::exit(0);
            }
            h
        }
    };

    // PID главного процесса для updater и --updated-перезапуска
    // (ожидание смерти / завершение владельца C5-mutex). Пишем при
    // каждом старте, до log::init — запись в файл лога не требует.
    // main_pid.txt updater удаляет на шаге cleanup; протухший PID
    // не страшен: wait сверяется со смертью процесса, а не с файлом.
    let pid_path =
        crate::update::tmp_dir(&crate::update::program_dir()).join(crate::update::MAIN_PID_FILE);
    if let Some(parent) = pid_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&pid_path, std::process::id().to_string());

    // Логи пишутся рядом с программой, с ограничением по объёму.
    crate::log::init(&crate::update::program_dir());
    crate::log::info(&format!("Tray Session {VERSION} запущена"));

    let cfg_handle = AppConfig::load();
    // Автоопределение SteamID как в Python-версии
    let mut cfg_handle = cfg_handle;
    if cfg_handle.steam_id.is_empty() || cfg_handle.steam_id == "auto" {
        if let Some(sid) = detector::local_steam_id(&detector::steam_install_path()) {
            cfg_handle.steam_id = sid;
            cfg_handle.save();
        }
    }

    let db = Arc::new(
        db::Db::open("sessions.db", cfg_handle.day_start_hour).expect("не могу открыть sessions.db"),
    );
    let games_vec = initial_games();
    let games: Arc<RwLock<Vec<config::TrackedGame>>> = Arc::new(RwLock::new(games_vec));
    let active: Arc<RwLock<std::collections::HashMap<String, monitor::ActiveInfo>>> =
        Arc::new(RwLock::new(Default::default()));
    let cfg: Arc<RwLock<AppConfig>> = Arc::new(RwLock::new(cfg_handle.clone()));

    let (tx_mon, rx_mon) = mpsc::channel::<monitor::MonitorEvent>();
    let (tx_tray, rx_tray) = mpsc::channel::<TrayCmd>();
    let (tx_app, rx_app) = mpsc::channel::<AppCmd>();

    let _mon_handle = monitor::spawn_monitor(games.clone(), active.clone(), cfg.clone(), tx_mon);
    spawn_tray(rx_tray, tx_app);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([720.0, 480.0])
            // Скрываем из taskbar: сворачивание = «уход в трей», не должно
            // оставлять кнопку в панели задач.
            .with_taskbar(false)
            // Та же иконка, что в трее: в заголовке окна была системная
            // картинка по умолчанию, и программа не выглядела «своей».
            .with_icon(egui::IconData {
                rgba: build_app_icon(32),
                width: 32,
                height: 32,
            }),
        ..Default::default()
    };
    eframe::run_native(
        "Tray Session",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(TrackerApp::new(db, games, active, cfg, rx_mon, rx_app, tx_tray)) as Box<dyn eframe::App>)
        }),
    )
}
