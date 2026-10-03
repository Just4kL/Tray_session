use chrono::{DateTime, Local, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn default_check_interval() -> u64 { 10 }
fn default_grace() -> u64 { 10 }
fn default_day_start() -> u32 { 3 }
fn default_true() -> bool { true }
fn default_vram() -> u64 { 80 }
fn default_cpu() -> f32 { 3.0 }
fn default_ram() -> u64 { 250 }
fn default_confirm() -> u32 { 1 }
/// Базовый масштаб: бывший 130% теперь считается за 100%.
fn default_scale() -> f32 { 1.3 }
fn default_opacity100() -> f32 { 100.0 }
fn default_strip_pos() -> u8 { 7 }
fn default_alarm_h() -> i32 { 8 }
fn default_timer_min() -> i32 { 10 }
/// Пресеты таймера по умолчанию (минуты): 1/5/10/15/30/60 — тот же набор,
/// что исторически был на вкладке «Таймер».
fn default_timer_presets() -> Vec<u32> { vec![1, 5, 10, 15, 30, 60] }
/// Границы одного пресета таймера в минутах (как у `timer_min`).
pub const TIMER_PRESET_MIN: u32 = 1;
pub const TIMER_PRESET_MAX: u32 = 1440;
/// Сколько пресетов максимум: больше не помещается в строку оверлея.
pub const TIMER_PRESETS_MAX: usize = 12;

/// Привести список пресетов к рабочему виду: зажать каждый в 1..=1440,
/// отсортировать по возрастанию, убрать дубли и обрезать до `TIMER_PRESETS_MAX`.
/// Пустой список заменяется умолчанием: без пресетов пропадает быстрый запуск
/// таймера и в оверлее, и на вкладке.
pub fn sanitize_timer_presets(raw: &[u32]) -> Vec<u32> {
    let mut v: Vec<u32> = raw
        .iter()
        .map(|m| (*m).clamp(TIMER_PRESET_MIN, TIMER_PRESET_MAX))
        .collect();
    v.sort_unstable();
    v.dedup();
    v.truncate(TIMER_PRESETS_MAX);
    if v.is_empty() {
        default_timer_presets()
    } else {
        v
    }
}
fn default_last_tab() -> String { "sessions".to_string() }
fn default_cols4() -> [bool; 4] { [true; 4] }
/// Как часто проверять обновления: 0 — каждый час (по умолчанию).
fn default_update_freq() -> u8 { 0 }
fn default_update_auto() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub steam_id: String,
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
    #[serde(default = "default_grace")]
    pub grace_secs: u64,
    #[serde(default = "default_day_start")]
    pub day_start_hour: u32,
    /// Главное требование: отталкиваться от нагрузки на видеокарту,
    /// чтобы не путать лаунчер с реально запущенной игрой.
    #[serde(default = "default_true")]
    pub require_gpu: bool,
    #[serde(default = "default_vram")]
    pub min_vram_mb: u64,
    #[serde(default = "default_cpu")]
    pub min_cpu_pct: f32,
    #[serde(default = "default_ram")]
    pub min_ram_mb: u64,
    /// Сколько подряд «активных» опросов нужно для старта сессии (анти-дребезг).
    #[serde(default = "default_confirm")]
    pub confirm_hits: u32,
    #[serde(default = "default_true")]
    pub auto_scan_enabled: bool,
    /// Масштаб интерфейса (для читаемости на больших мониторах). 1.0 = 100%.
    #[serde(default = "default_scale")]
    pub ui_scale: f32,
    /// Инфопанель сессии: сверху (true) или снизу (false) экрана.
    #[serde(default = "default_true")]
    pub infobar_top: bool,
    /// Steam API хоть раз успешно опрашивался: тогда отсутствие минут
    /// у игры = её нет в Steam (не куплена/удалена), иначе — неизвестно.
    #[serde(default)]
    pub steam_synced: bool,
    /// Запомненные позиции плавающих окон (верхний левый угол, поинты).
    /// Восстанавливаются при открытии, если точка всё ещё на живом мониторе.
    #[serde(default)]
    pub stopwatch_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub strip_pos_manual: Option<[f32; 2]>,
    /// Позиция главного окна (C4): запоминается при перемещении,
    /// восстанавливается при старте через ViewportBuilder::with_position.
    #[serde(default)]
    pub main_window_pos: Option<[f32; 2]>,
    // --- Состояние интерфейса (автозапоминание всех изменений пользователя) ---
    /// Видимость колонок таблицы сессий: №, %, запускал, сессий.
    #[serde(default = "default_cols4")]
    pub show_cols: [bool; 4],
    #[serde(default = "default_true")]
    pub show_analog: bool,
    #[serde(default)]
    pub strip_open: bool,
    #[serde(default = "default_strip_pos")]
    pub strip_pos: u8,
    #[serde(default)]
    pub strip_pinned: bool,
    #[serde(default = "default_opacity100")]
    pub strip_opacity_pct: f32,
    #[serde(default)]
    pub stopwatch_overlay: bool,
    #[serde(default = "default_opacity100")]
    pub stopwatch_opacity_pct: f32,
    /// Окошко секундомера развёрнуто: двойной размер + таймер обратного отсчёта.
    #[serde(default)]
    pub stopwatch_expanded: bool,
    #[serde(default = "default_alarm_h")]
    pub alarm_h: i32,
    #[serde(default)]
    pub alarm_m: i32,
    #[serde(default)]
    pub alarm_days: [bool; 7],
    #[serde(default = "default_timer_min")]
    pub timer_min: i32,
    /// Пресеты быстрого запуска таймера (минуты). Показываются на вкладке
    /// «Таймер» и в развёрнутом оверлее секундомера. Порядок — по возрастанию;
    /// дубли и выход за 1..=1440 отсекаются при загрузке (T-17).
    #[serde(default = "default_timer_presets")]
    pub timer_presets: Vec<u32>,
    /// Последняя открытая вкладка.
    #[serde(default = "default_last_tab")]
    pub last_tab: String,
    // --- Обновления ---
    /// Как часто проверять свежие сборки: 0 — каждый час, 1 — ежедневно,
    /// 2 — раз в неделю, 3 — вручную (код хранится числом, чтобы старые
    /// config.json не ломались при добавлении вариантов).
    #[serde(default = "default_update_freq")]
    pub update_freq: u8,
    /// Проверять обновления автоматически (false — только по кнопке).
    #[serde(default = "default_update_auto")]
    pub update_auto: bool,
    /// Молча скачивать и ставить обновления, не спрашивая. По умолчанию
    /// false: программа сама скачает и заменит файлы только с согласия.
    #[serde(default)]
    pub update_silent: bool,
    /// Когда последний раз проверяли обновления (unix-секунды). Нужно,
    /// чтобы не проверять на каждом запуске.
    #[serde(default)]
    pub update_last_check: i64,
    /// Канал обновлений: `stable` или `beta`.
    ///
    /// Хранится строкой, а не числом: читается человеком в config.json
    /// при разборе, что случилось с программой. Неизвестное значение
    /// считается стабильным (см. `update::normalize_channel`), чтобы
    /// опечатка не отключила обновления совсем.
    #[serde(default = "default_update_channel")]
    pub update_channel: String,
}

fn default_update_channel() -> String {
    "stable".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            steam_id: String::new(),
            check_interval_secs: default_check_interval(),
            grace_secs: default_grace(),
            day_start_hour: default_day_start(),
            require_gpu: true,
            min_vram_mb: default_vram(),
            min_cpu_pct: default_cpu(),
            min_ram_mb: default_ram(),
            confirm_hits: default_confirm(),
            auto_scan_enabled: true,
            ui_scale: default_scale(),
            infobar_top: true,
            steam_synced: false,
            stopwatch_pos: None,
            strip_pos_manual: None,
            main_window_pos: None,
            show_cols: [true; 4],
            show_analog: true,
            strip_open: false,
            strip_pos: 7,
            strip_pinned: false,
            strip_opacity_pct: 100.0,
            stopwatch_overlay: false,
            stopwatch_opacity_pct: 100.0,
            stopwatch_expanded: false,
            alarm_h: 8,
            alarm_m: 0,
            alarm_days: [false; 7],
            timer_min: 10,
            timer_presets: default_timer_presets(),
            last_tab: "sessions".to_string(),
            update_freq: default_update_freq(),
            update_auto: default_update_auto(),
            update_silent: false,
            update_last_check: 0,
            update_channel: default_update_channel(),
        }
    }
}

impl AppConfig {
    pub fn path() -> PathBuf {
        PathBuf::from("config.json")
    }
    /// Прочитать настройки из файла по умолчанию.
    pub fn load() -> Self {
        Self::load_from(Self::path())
    }

    /// Прочитать настройки из указанного файла.
    ///
    /// Отдельная функция нужна тестам: путь по умолчанию относительный, а
    /// тесты идут параллельно и не могут менять рабочую папку — иначе один
    /// тест читался бы из файла, который переписал другой.
    pub fn load_from<P: AsRef<Path>>(path: P) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => {
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(v) => {
                        let mut cfg = Self::default();
                        if let Some(s) = v.get("api_key").and_then(|x| x.as_str()) {
                            cfg.api_key = s.to_string();
                        }
                        if let Some(s) = v.get("steam_id").and_then(|x| x.as_str()) {
                            cfg.steam_id = s.to_string();
                        }
                        if let Some(n) = v.get("check_interval_secs").and_then(|x| x.as_u64()) {
                            cfg.check_interval_secs = n.clamp(2, 120);
                        }
                        if let Some(n) = v.get("grace_secs").and_then(|x| x.as_u64()) {
                            cfg.grace_secs = n.clamp(2, 120);
                        }
                        if let Some(n) = v.get("day_start_hour").and_then(|x| x.as_u64()) {
                            cfg.day_start_hour = (n as u32).min(23);
                        }
                        if let Some(b) = v.get("require_gpu").and_then(|x| x.as_bool()) {
                            cfg.require_gpu = b;
                        }
                        if let Some(n) = v.get("min_vram_mb").and_then(|x| x.as_u64()) {
                            cfg.min_vram_mb = n;
                        }
                        if let Some(n) = v.get("min_cpu_pct").and_then(|x| x.as_f64()) {
                            cfg.min_cpu_pct = n as f32;
                        }
                        if let Some(n) = v.get("min_ram_mb").and_then(|x| x.as_u64()) {
                            cfg.min_ram_mb = n;
                        }
                        if let Some(n) = v.get("confirm_hits").and_then(|x| x.as_u64()) {
                            cfg.confirm_hits = (n as u32).clamp(1, 10);
                        }
                        if let Some(b) = v.get("auto_scan_enabled").and_then(|x| x.as_bool()) {
                            cfg.auto_scan_enabled = b;
                        }
                        if let Some(n) = v.get("ui_scale").and_then(|x| x.as_f64()) {
                            cfg.ui_scale = (n as f32).clamp(0.8, 2.0);
                        }
                        if let Some(b) = v.get("infobar_top").and_then(|x| x.as_bool()) {
                            cfg.infobar_top = b;
                        }
                        if let Some(b) = v.get("steam_synced").and_then(|x| x.as_bool()) {
                            cfg.steam_synced = b;
                        }
                        for (key, slot) in [
                            ("stopwatch_pos", &mut cfg.stopwatch_pos),
                            ("strip_pos_manual", &mut cfg.strip_pos_manual),
                        ] {
                            if let Some(a) = v.get(key).and_then(|x| x.as_array()) {
                                if a.len() == 2 {
                                    if let (Some(x), Some(y)) =
                                        (a[0].as_f64(), a[1].as_f64())
                                    {
                                        let (x, y) = (x as f32, y as f32);
                                        if x.is_finite() && y.is_finite() {
                                            *slot = Some([x, y]);
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(a) = v.get("show_cols").and_then(|x| x.as_array()) {
                            let mut c = [true; 4];
                            for (i, v) in a.iter().take(4).enumerate() {
                                if let Some(b) = v.as_bool() {
                                    c[i] = b;
                                }
                            }
                            cfg.show_cols = c;
                        }
                        if let Some(b) = v.get("show_analog").and_then(|x| x.as_bool()) {
                            cfg.show_analog = b;
                        }
                        if let Some(b) = v.get("strip_open").and_then(|x| x.as_bool()) {
                            cfg.strip_open = b;
                        }
                        if let Some(n) = v.get("strip_pos").and_then(|x| x.as_u64()) {
                            cfg.strip_pos = (n as u8).min(8);
                        }
                        if let Some(b) = v.get("strip_pinned").and_then(|x| x.as_bool()) {
                            cfg.strip_pinned = b;
                        }
                        if let Some(n) = v.get("strip_opacity_pct").and_then(|x| x.as_f64()) {
                            cfg.strip_opacity_pct = (n as f32).clamp(5.0, 100.0);
                        }
                        if let Some(b) = v.get("stopwatch_overlay").and_then(|x| x.as_bool()) {
                            cfg.stopwatch_overlay = b;
                        }
                        if let Some(n) = v.get("stopwatch_opacity_pct").and_then(|x| x.as_f64()) {
                            cfg.stopwatch_opacity_pct = (n as f32).clamp(2.0, 100.0);
                        }
                        if let Some(b) = v.get("stopwatch_expanded").and_then(|x| x.as_bool()) {
                            cfg.stopwatch_expanded = b;
                        }
                        if let Some(n) = v.get("alarm_h").and_then(|x| x.as_i64()) {
                            cfg.alarm_h = (n as i32).clamp(0, 23);
                        }
                        if let Some(n) = v.get("alarm_m").and_then(|x| x.as_i64()) {
                            cfg.alarm_m = (n as i32).clamp(0, 59);
                        }
                        if let Some(a) = v.get("alarm_days").and_then(|x| x.as_array()) {
                            let mut d = [false; 7];
                            for (i, v) in a.iter().take(7).enumerate() {
                                if let Some(b) = v.as_bool() {
                                    d[i] = b;
                                }
                            }
                            cfg.alarm_days = d;
                        }
                        if let Some(n) = v.get("timer_min").and_then(|x| x.as_i64()) {
                            cfg.timer_min = (n as i32).clamp(1, 1440);
                        }
                        // Пресеты таймера (T-17). Ключа может не быть в старом
                        // config.json — тогда остаётся умолчание. Мусор в
                        // массиве не должен доезжать до UI: чистим сразу.
                        if let Some(a) = v.get("timer_presets").and_then(|x| x.as_array()) {
                            let raw: Vec<u32> = a
                                .iter()
                                .filter_map(|x| x.as_u64())
                                .map(|n| n.min(TIMER_PRESET_MAX as u64) as u32)
                                .collect();
                            cfg.timer_presets = sanitize_timer_presets(&raw);
                        }
                        if let Some(s) = v.get("last_tab").and_then(|x| x.as_str()) {
                            cfg.last_tab = s.to_string();
                        }
                        // Настройки обновлений. Раньше они вообще не читались
                        // при загрузке: человек выбирал частоту или канал,
                        // нажимал «Сохранить» — а после перезапуска всё
                        // возвращалось к умолчанию молча.
                        if let Some(n) = v.get("update_freq").and_then(|x| x.as_u64()) {
                            cfg.update_freq = (n as u8).min(3);
                        }
                        if let Some(b) = v.get("update_auto").and_then(|x| x.as_bool()) {
                            cfg.update_auto = b;
                        }
                        if let Some(b) = v.get("update_silent").and_then(|x| x.as_bool()) {
                            cfg.update_silent = b;
                        }
                        if let Some(n) = v.get("update_last_check").and_then(|x| x.as_i64()) {
                            cfg.update_last_check = n;
                        }
                        if let Some(s) = v.get("update_channel").and_then(|x| x.as_str()) {
                            // Нормализуем сразу при чтении: мусор в файле не
                            // должен доезжать до выбора ветки.
                            cfg.update_channel =
                                crate::update::normalize_channel(s).to_string();
                        }
                        cfg
                    }
                    Err(_) => Self::default(),
                }
            }
            Err(_) => Self::default(),
        }
    }
    /// Сохранить настройки в файл по умолчанию.
    pub fn save(&self) {
        self.save_to(Self::path());
    }

    /// Сохранить настройки в указанный файл (тесты пишут в свою папку).
    pub fn save_to<P: AsRef<Path>>(&self, path: P) {
        let m = serde_json::json!({
            "api_key": self.api_key,
            "steam_id": self.steam_id,
            "check_interval_secs": self.check_interval_secs,
            "grace_secs": self.grace_secs,
            "day_start_hour": self.day_start_hour,
            "require_gpu": self.require_gpu,
            "min_vram_mb": self.min_vram_mb,
            "min_cpu_pct": self.min_cpu_pct,
            "min_ram_mb": self.min_ram_mb,
            "confirm_hits": self.confirm_hits,
            "auto_scan_enabled": self.auto_scan_enabled,
            "ui_scale": self.ui_scale,
            "infobar_top": self.infobar_top,
            "steam_synced": self.steam_synced,
            "stopwatch_pos": self.stopwatch_pos,
            "strip_pos_manual": self.strip_pos_manual,
            "show_cols": self.show_cols,
            "show_analog": self.show_analog,
            "strip_open": self.strip_open,
            "strip_pos": self.strip_pos,
            "strip_pinned": self.strip_pinned,
            "strip_opacity_pct": self.strip_opacity_pct,
            "stopwatch_overlay": self.stopwatch_overlay,
            "stopwatch_opacity_pct": self.stopwatch_opacity_pct,
            "stopwatch_expanded": self.stopwatch_expanded,
            "alarm_h": self.alarm_h,
            "alarm_m": self.alarm_m,
            "alarm_days": self.alarm_days,
            "timer_min": self.timer_min,
            "timer_presets": self.timer_presets,
            "last_tab": self.last_tab,
            // Настройки обновлений. Их не было в списке: человек менял
            // частоту или канал, нажимал «Сохранить», а файл их не содержал
            // — при перезапуске всё возвращалось к умолчанию без всякого
            // сообщения.
            "update_freq": self.update_freq,
            "update_auto": self.update_auto,
            "update_silent": self.update_silent,
            "update_last_check": self.update_last_check,
            "update_channel": self.update_channel,
        });
        let _ = fs::write(path, serde_json::to_string_pretty(&m).unwrap_or_default());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedGame {
    pub name: String,
    pub exe_path: String,
    #[serde(default = "default_source")]
    pub source: String,
    /// Всего минут по серверам Steam (playtime_forever). None = данных нет
    /// (не в Steam / не куплена / удалена / API ещё не опрашивали).
    /// Наши сессии — дельты, прибавляемые к этому общему.
    #[serde(default)]
    pub steam_minutes: Option<u64>,
}

fn default_source() -> String { "Manual".to_string() }

impl TrackedGame {
    pub fn key(&self) -> String {
        normalize_exe(&self.exe_path)
    }
    pub fn file_name(&self) -> String {
        exe_file_name(&self.exe_path)
    }
}

pub fn normalize_exe(p: &str) -> String {
    p.replace('/', "\\").to_lowercase()
}

pub fn exe_file_name(p: &str) -> String {
    p.replace('/', "\\")
        .rsplit('\\')
        .next()
        .unwrap_or(p)
        .to_lowercase()
}

const KNOWN_GAMES_FILE: &str = "known_games.json";

pub fn load_known_games() -> Vec<TrackedGame> {
    match fs::read_to_string(KNOWN_GAMES_FILE) {
        Ok(text) => serde_json::from_str::<Vec<TrackedGame>>(&text).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_known_games(games: &[TrackedGame]) {
    let _ = fs::write(
        KNOWN_GAMES_FILE,
        serde_json::to_string_pretty(games).unwrap_or_default(),
    );
}

pub fn merge_games(lists: Vec<Vec<TrackedGame>>) -> Vec<TrackedGame> {
    let mut map: HashMap<String, TrackedGame> = HashMap::new();
    // Порядок приоритета: ручные должны побеждать, поэтому их кладём последними.
    // Но для простоты: последний встреченный перезаписывает, а вызывающий код
    // передаёт списки от низкого приоритета к высокому.
    for list in lists {
        for g in list {
            map.insert(g.key(), g);
        }
    }
    map.into_values().collect()
}

pub fn game_day_key(dt: &DateTime<Local>, day_start_hour: u32) -> String {
    let d = if dt.format("%H").to_string().parse::<u32>().unwrap_or(0) < day_start_hour {
        dt.date_naive() - Duration::days(1)
    } else {
        dt.date_naive()
    };
    d.format("%Y-%m-%d").to_string()
}

pub fn format_duration(total_secs: i64) -> String {
    let s = total_secs.max(0);
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    format!("{h} ч : {m} м : {sec} с")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_presets_default_is_the_historical_set() {
        assert_eq!(default_timer_presets(), vec![1, 5, 10, 15, 30, 60]);
        assert_eq!(AppConfig::default().timer_presets, vec![1, 5, 10, 15, 30, 60]);
    }

    #[test]
    fn sanitize_timer_presets_sorts_dedups_and_clamps() {
        // Порядок не важен — приводим к возрастанию.
        assert_eq!(sanitize_timer_presets(&[30, 1, 10]), vec![1, 10, 30]);
        // Дубли схлопываются.
        assert_eq!(sanitize_timer_presets(&[5, 5, 1, 5]), vec![1, 5]);
        // Ноль и перебор зажимаются в 1..=1440.
        assert_eq!(sanitize_timer_presets(&[0, 5000]), vec![1, 1440]);
        // Пустой список — не «нет пресетов», а умолчание.
        assert_eq!(sanitize_timer_presets(&[]), default_timer_presets());
        // Хвост сверх лимита отрезается (после сортировки — самые крупные).
        let many: Vec<u32> = (1..=20).collect();
        assert_eq!(sanitize_timer_presets(&many).len(), TIMER_PRESETS_MAX);
    }

    #[test]
    fn timer_presets_survive_a_save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("gst_cfg_presets_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("config.json");
        let cfg = AppConfig {
            timer_presets: vec![2, 45, 90],
            ..Default::default()
        };
        cfg.save_to(&path);
        assert_eq!(AppConfig::load_from(&path).timer_presets, vec![2, 45, 90]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_broken_timer_presets_fall_back_to_default() {
        let dir = std::env::temp_dir().join(format!("gst_cfg_bad_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        // Ключа нет вовсе — умолчание (старый config.json не ломается).
        let p1 = dir.join("a.json");
        fs::write(&p1, "{\"timer_min\": 7}").unwrap();
        assert_eq!(
            AppConfig::load_from(&p1).timer_presets,
            default_timer_presets()
        );
        // Ключ есть, но валидных чисел в нём нет — тоже умолчание.
        let p2 = dir.join("b.json");
        fs::write(&p2, "{\"timer_presets\": [\"abc\", null, -5]}").unwrap();
        assert_eq!(
            AppConfig::load_from(&p2).timer_presets,
            default_timer_presets()
        );
        // Смешанный массив: валидные числа берём, мусор игнорируем.
        let p3 = dir.join("c.json");
        fs::write(&p3, "{\"timer_presets\": [15, \"x\", 5]}").unwrap();
        assert_eq!(AppConfig::load_from(&p3).timer_presets, vec![5, 15]);
        let _ = fs::remove_dir_all(&dir);
    }
}
