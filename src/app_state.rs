//! Бизнес-состояние приложения.
//!
//! Выделено из `app.rs` на этапе 3.0 модульного UI: `TrackerApp` хранит
//! `state: AppState` плюс контейнеры представления (`Views`,
//! `ThemeManager`), а все методы `impl TrackerApp` обращаются к полям
//! через `self.state.*`.
//!
//! Правило раздела: сюда — всё, что уходит в БД/конфиг или переживает
//! кадр по смыслу (сессии, будильники, таймер, обновления). Локальное
//! UI-состояние (фильтры, черновики, раскрытые группы) в фазе 3
//! переедет из `AppState` в отдельные `Views`, по одной вкладке за
//! коммит. Пока всё лежит здесь — поведение 1-в-1.
//!
//! Сюда же переехали `Tab`, `AppCmd`, `TrayCmd`, `UpdateInfo`: это
//! сообщения и состояние уровня приложения, а не рисование. `main.rs`
//! берёт `AppCmd`/`TrayCmd` отсюда, а не из `app`.

use crate::config::AppConfig;
use crate::db::{AggRow, AlarmRow, Db};
use crate::detector;
use crate::monitor::{MonitorEvent, SharedActive, SharedConfig, SharedGames};
use crate::shortcuts::{HotkeyReg, ShortcutStore};
use crate::sound::SoundPlayer;
use std::collections::HashMap;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Вкладка главного окна.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Sessions,
    Games,
    Alarms,
    Timer,
    Shortcuts,
    About,
}

impl Tab {
    pub fn as_str(self) -> &'static str {
        match self {
            Tab::Sessions => "sessions",
            Tab::Games => "games",
            Tab::Alarms => "alarms",
            Tab::Timer => "timer",
            Tab::Shortcuts => "shortcuts",
            Tab::About => "about",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "games" => Tab::Games,
            "alarms" => Tab::Alarms,
            "timer" => Tab::Timer,
            "shortcuts" => Tab::Shortcuts,
            "about" => Tab::About,
            _ => Tab::Sessions,
        }
    }
}

/// Команды главному окну от трея и хоткеев.
#[derive(Debug, Clone)]
pub enum AppCmd {
    Show,
    Quit,
    StopwatchToggle,
    StopwatchLap,
    StopwatchStop,
    StopwatchPin,
    /// Шаг прозрачности окошка секундомера (+5/-5), только когда Pinned.
    OpacityDelta(i32),
    /// Сдвиг полоски по сетке 3×3: −1 = к предыдущей позиции, +1 = к следующей.
    StripStep(i32),
    /// Прозрачность мини-трея ±5 (стрелки RShift+↑/↓ при закрытом секундомере).
    StripOpacity(i32),
}

/// Команды трею от главного окна.
#[derive(Debug, Clone)]
pub enum TrayCmd {
    Tooltip(String),
    /// Глобальные хоткеи секундомера (пусто = снять все).
    StopwatchHotkeys(Vec<HotkeyReg>),
}

/// Ответ фоновой нити, которая проверяет обновления.
#[derive(Debug, Clone, Default)]
pub struct UpdateInfo {
    /// Текст для раздела «О программе».
    pub msg: String,
    /// Версия найденного обновления, если оно есть. Пока она задана,
    /// в правом верхнем углу показывается кнопка скачивания.
    pub new_version: Option<String>,
}

/// Бизнес-состояние приложения.
///
/// Поля сгруппированы как в исходном `TrackerApp`: общие, sessions ui,
/// games ui, process picker, folder candidates, alarms ui, timer ui,
/// misc. Все поля `pub`: представления в `ui::views` живут в другом
/// модуле и берут их через `&mut AppState`.
pub struct AppState {
    pub db: Arc<Db>,
    pub games: SharedGames,
    pub active: SharedActive,
    pub cfg: SharedConfig,
    pub cfg_handle: AppConfig,
    pub rx_monitor: mpsc::Receiver<MonitorEvent>,
    pub rx_appcmd: mpsc::Receiver<AppCmd>,
    pub tx_tray: mpsc::Sender<TrayCmd>,
    pub tab: Tab,
    pub agg: Vec<AggRow>,
    pub last_agg_refresh: Instant,
    pub session_id_by_exe: HashMap<String, i64>,
    // sessions ui
    pub show_id_col: bool,
    pub show_pct_col: bool,
    pub show_last_col: bool,
    pub show_runs_col: bool,
    pub detail_game: Option<String>,
    pub edit_session: Option<(i64, i32, i32, i32)>, // id,h,m,s
    // games ui
    pub search: String,
    pub api_key: String,
    pub steam_id: String,
    pub scan_msg: String,
    // process picker
    pub show_picker: bool,
    pub picker_search: String,
    pub picker_list: Vec<detector::ProcInfo>,
    pub picker_refresh_at: Instant,
    // folder candidates
    pub show_folder: bool,
    pub folder_candidates: Vec<(String, u64)>,
    pub folder_name: String,
    // alarms ui
    pub alarms: Vec<AlarmRow>,
    pub alarm_h: i32,
    pub alarm_m: i32,
    /// Дни недели нового будильника галочками (Пн..Вс). Пусто = разовый.
    pub alarm_days: [bool; 7],
    pub alarm_url: String,
    pub alarm_sound: String,
    pub alarm_std: String,
    pub alarm_dialog: Option<AlarmRow>,
    /// Редактирование будильника (id, часы, минуты, дни Пн..Вс).
    /// Доступно в любой момент, в том числе для неактивных будильников.
    pub edit_alarm: Option<(i64, i32, i32, [bool; 7])>,
    /// Показывать ли аналоговые часы для активного будильника.
    pub show_analog: bool,
    /// Какой будильник выведен на аналоговые часы (id). None = ближайший.
    pub analog_alarm_id: Option<i64>,
    // timer ui
    pub timer_url: String,
    pub timer_sound: String,
    pub timer_std: String,
    pub timer_min: i32,
    pub timer_end: Option<Instant>,
    pub timer_total: u64,
    pub timer_data: (Option<String>, Option<String>, Option<String>),
    pub timer_dialog: bool,
    // misc
    pub sound: SoundPlayer,
    pub last_alarm_min: String,
    pub morning_shown: bool,
    pub started_at: Instant,
    pub last_autoscan: Instant,
    pub quit_requested: bool,
    pub gpu_usable_cache: bool,
    pub gpu_util_cache: u32,
    pub last_gpu_check: Instant,
    pub status_msg: String,
    /// Черновик масштаба интерфейса: правится ползунком без перестройки UI,
    /// применяется (commit_scale) при отпускании ползунка или кнопками.
    pub scale_draft: Option<f32>,
    /// Черновики настроек обновлений: правятся в UI без записи в конфиг,
    /// применяются кнопкой «Сохранить настройки обновлений». Паттерн как
    /// у scale_draft: без черновика ComboBox писал бы в клон, умиравший
    /// в конце кадра, и выбор не сохранялся бы (баг C11).
    pub update_freq_draft: Option<u8>,
    pub update_auto_draft: Option<bool>,
    pub update_silent_draft: Option<bool>,
    pub update_channel_draft: Option<String>,
    /// Пользовательские шорткаты (shortcuts.json, автосохранение).
    pub shortcuts: ShortcutStore,
    /// Захват нового шортката: id действия + время старта (анти-дребезг).
    pub capture_action: Option<String>,
    pub capture_armed_at: f64,
    /// Быстрый будильник (Ctrl+}): разовый, звук BEEP.
    pub quick_alarm_open: bool,
    pub quick_alarm_text: String,
    pub quick_alarm_focus: bool,
    /// Быстрый таймер (Ctrl+{): минуты 1–999, Enter — старт.
    pub quick_timer_open: bool,
    pub quick_timer_text: String,
    pub quick_timer_focus: bool,
    /// Секундомер: идёт ли отсчёт + накопленное время + круги (до 99 за забег).
    pub stopwatch_running: bool,
    pub stopwatch_start: Option<Instant>,
    pub stopwatch_base: Duration,
    pub stopwatch_laps: Vec<Duration>,
    /// Маленькое окошко секундомера поверх всех окон + его прозрачность 2–100%.
    pub stopwatch_overlay: bool,
    pub stopwatch_opacity_pct: f32,
    /// Окошко секундомера развёрнуто (кнопка расширения): двойной размер
    /// плюс таймер обратного отсчёта. Запоминается, как и прочие настройки.
    pub stopwatch_expanded: bool,
    /// Последняя командная позиция окошка при перетаскивании (анти-лаг).
    pub stopwatch_drag_pos: Option<egui::Pos2>,
    /// Поставить окошко на запомненную позицию первым кадром.
    pub overlay_place_pending: bool,
    /// ppp при прошлой синхронизации рамок плавающих окон.
    pub last_ppp: f32,
    /// Смещение скролла списка кругов (кнопки ▲▼ + колесо).
    pub laps_scroll: f32,
    /// Тонкая полоска сессии (стиль uTorrent): открыта / позиция 0–8 (3×3) /
    /// drag / закреплена (PIN) / прозрачность 5–100%.
    pub strip_open: bool,
    pub strip_pos: u8,
    pub strip_drag_pos: Option<egui::Pos2>,
    pub strip_pinned: bool,
    pub strip_opacity_pct: f32,
    /// Отложить прижатие полоски к краю до первого кадра вьюпорта.
    pub strip_snap_pending: bool,
    /// Когда последний раз проверяли, что окна не уехали за границы экрана
    /// (мониторы могли переподключиться или сменить разрешение).
    pub last_edge_check: Instant,
    /// Текущий размер окошка секундомера в пикселях — общий источник правды
    /// для проверок границ экрана (не пересчитываем на месте).
    pub last_stopwatch_size: egui::Vec2,
    /// PIN: окно нельзя перетащить; Ctrl+Num+/Num− меняют прозрачность ±5%.
    pub stopwatch_pinned: bool,
    /// Тонкая инфопанель игровой сессии (стиль uTorrent) + её размещение.
    pub infobar_open: bool,
    pub infobar_placed: bool,
    /// Проверка обновлений: приёмник результата, статус, флаг процесса.
    pub update_rx: Option<std::sync::mpsc::Receiver<UpdateInfo>>,
    pub update_status: String,
    pub update_checking: bool,
    /// Момент, когда была запущена установка обновления.
    /// Сторож в update(): если через N секунд не пришёл ok=true
    /// от апдейтера — откатить quit_requested и показать ошибку.
    /// Заодно работает флагом «установка идёт»: кнопка «Установить сейчас»
    /// заблокирована, пока он задан (баг C12 — повторный клик плодил
    /// второй апдейтер).
    pub update_pending_since: Option<std::time::Instant>,
    /// Версия найденного обновления, если оно есть. Пока задана — в правом
    /// верхнем углу висит жёлтая кнопка со стрелкой вниз.
    pub update_available: Option<String>,
    /// Момент запуска проверки обновлений (время egui) — для защиты от
    /// повторного автозапуска при каждом кадре раздела «О программе».
    pub update_window_opened_at: f64,
    /// Последний отправленный в трей набор глобальных хоткеев (без дублей).
    pub last_hotkeys: Vec<HotkeyReg>,
    /// Последний тултип трея (вместо static mut).
    pub last_tooltip: Instant,
    /// Подраздел «О программе» (0–3).
    pub about_sub: u8,
    /// Последний сохранённый в файл слепок конфига (для автоперсиста).
    pub last_saved: AppConfig,
    /// Вкладка прошлого кадра: при переходе на Сессии/Будильники обновляем данные.
    pub prev_tab: Tab,
}
