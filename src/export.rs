//! Выгрузка таблицы игровых сессий в CSV.
//!
//! Модуль общий для программы и для деактиватора: раньше выгрузка жила в
//! `app.rs` и была завязана на данные в памяти, из-за чего деактиватор не
//! мог сохранить таблицу перед удалением. Теперь CSV собирается прямо из
//! `sessions.db` одним и тем же кодом, так что файлы получаются
//! одинаковыми.
//!
//! Деактиватор пользуется этим, чтобы не потерять историю: перед удалением
//! он предлагает выгрузить таблицу, а затем открывает Проводник на неё.

use std::path::Path;

/// Строка сводки по игре, готовая к выгрузке.
#[derive(Debug, Clone, PartialEq)]
pub struct CsvRow {
    pub game: String,
    /// Общее время в секундах.
    pub total: i64,
    /// Доля от суммы всех игр, в процентах.
    pub percent: f64,
    /// Когда запускали последний раз (уже отформатировано) или «—».
    pub last: String,
    /// Сколько раз запускали.
    pub runs: i64,
    /// Сколько сессий.
    pub sessions: i64,
}

/// Форматировать секунды как «ч:мм:сс», как принято в интерфейсе.
pub fn format_duration(secs: i64) -> String {
    let neg = secs < 0;
    let s = secs.abs();
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    let body = if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    };
    if neg { format!("-{body}") } else { body }
}

/// Собрать строки сводки из готовых данных.
///
/// `rows` — как из `db::aggregated()`: (игра, общее время, последний запуск
/// в RFC3339, запусков, сессий).
pub fn build_rows(rows: &[(String, i64, String, i64, i64)]) -> Vec<CsvRow> {
    let total_all: i64 = rows.iter().map(|r| r.1).sum();
    rows.iter()
        .map(|r| CsvRow {
            game: r.0.clone(),
            total: r.1,
            percent: if total_all > 0 {
                r.1 as f64 / total_all as f64 * 100.0
            } else {
                0.0
            },
            last: if r.2.is_empty() {
                "—".to_string()
            } else {
                match chrono::DateTime::parse_from_rfc3339(&r.2) {
                    Ok(d) => d
                        .with_timezone(&chrono::Local)
                        .format("%H:%M %Y-%m-%d")
                        .to_string(),
                    Err(_) => r.2.clone(),
                }
            },
            runs: r.3,
            sessions: r.4,
        })
        .collect()
}

/// Записать сводку в CSV-файл. Возвращает число строк данных.
pub fn write_csv(path: &Path, rows: &[CsvRow]) -> Result<usize, String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("не создана папка {}: {e}", d.display()))?;
    }
    let file =
        std::fs::File::create(path).map_err(|e| format!("не создан {}: {e}", path.display()))?;
    let mut w = csv::Writer::from_writer(file);
    w.write_record(["№", "Игра", "Общее время", "% от общего", "Запускал", "Сессий"])
        .map_err(|e| format!("ошибка записи: {e}"))?;
    for (i, r) in rows.iter().enumerate() {
        w.write_record([
            (i + 1).to_string(),
            r.game.clone(),
            format_duration(r.total),
            format!("{:.2}%", r.percent),
            r.last.clone(),
            r.sessions.to_string(),
        ])
        .map_err(|e| format!("ошибка записи: {e}"))?;
    }
    w.flush().map_err(|e| format!("не дописан файл: {e}"))?;
    Ok(rows.len())
}

/// Папка для выгрузки по умолчанию: Рабочий стол, если он есть, иначе
/// временная.
///
/// Не рядом с программой: при удалении папка с выгрузкой сносится, и
/// человек теряет таблицу вместе с программой.
pub fn default_export_dir() -> std::path::PathBuf {
    let desk = std::env::var("USERPROFILE")
        .map(|p| std::path::PathBuf::from(p).join("Desktop"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    if desk.is_dir() {
        return desk;
    }
    std::env::var("TEMP")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Свободное имя выгрузки в папке `dir`: `sessions.csv`, а если он уже
/// занят — `sessions_2.csv`, `sessions_3.csv` и так далее.
///
/// Без этого выгрузка при деактивации затирала бы прошлую таблицу, и
/// человек потерял бы историю вместе с новой выгрузкой.
pub fn unique_path(dir: &Path) -> std::path::PathBuf {
    let first = dir.join("sessions.csv");
    if !first.exists() {
        return first;
    }
    for n in 2..10_000 {
        let p = dir.join(format!("sessions_{n}.csv"));
        if !p.exists() {
            return p;
        }
    }
    // Крайний случай: имена кончились. Не перетираем чужой файл.
    dir.join(format!("sessions_{}.csv", std::process::id()))
}

/// Куда класть выгрузку по умолчанию (папка + свободное имя).
pub fn default_export_path() -> std::path::PathBuf {
    unique_path(&default_export_dir())
}

/// Открыть Проводник на указанном файле, чтобы человек сразу увидел
/// сохранённую таблицу.
pub fn reveal_in_explorer(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
#[path = "export/tests.rs"]
mod tests;
