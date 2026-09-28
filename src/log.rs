//! Логи программы: файл в папке `logs` рядом с программой, с ограничением
//! по объёму.
//!
//! Зачем ограничение: логи пишутся рядом с программой и растут без
//! остановки, а на диске C: места мало. Поэтому действуют два правила:
//!
//! 1. Текущий лог переворачивается (`tray.log` -> `tray.1.log` ->
//!    `tray.2.log` ...), пока не станет больше `ROTATE_BYTES`.
//! 2. Если суммарный объём папки превысил `MAX_TOTAL_BYTES`, более старые
//!    файлы удаляются, начиная с самых старых, пока лимит не будет
//!    соблюдён. Так логи не могут съесть диск.
//!
//! Логирование никогда не должно ронять программу: если папку создать или
//! файл открыть не удалось (например, программа в Program Files без прав),
//! запись молча пропускается, а программа работает дальше.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Максимальный объём одного файла лога. После превышения он переворачивается.
pub const ROTATE_BYTES: u64 = 512 * 1024;
/// Максимальный суммарный объём папки логов.
pub const MAX_TOTAL_BYTES: u64 = 10 * 1024 * 1024;
/// Сколько файлов держим при переворачивании (1 = текущий + 0 старых).
const KEEP_FILES: usize = 40;

static STATE: Mutex<Option<Logger>> = Mutex::new(None);

/// Состояние логгера.
struct Logger {
    dir: PathBuf,
    file: PathBuf,
    written: u64,
}

/// Один файл лога на диске.
struct LogFile {
    path: PathBuf,
    size: u64,
    modified: std::time::SystemTime,
}

impl Logger {
    fn new(dir: PathBuf) -> Option<Logger> {
        std::fs::create_dir_all(&dir).ok()?;
        let file = dir.join("tray.log");
        let written = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        Some(Logger { dir, file, written })
    }

    /// Записать строку. Ошибки молча игнорируются: лог не должен мешать.
    fn write(&mut self, level: &str, msg: &str) {
        // Каждая строка пишется целиком — иначе в логе рвутся строки.
        let line = format!("{} [{}] {}\n", timestamp(), level, single_line(msg));
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)
        {
            let _ = f.write_all(line.as_bytes());
        }
        self.written += line.len() as u64;
        if self.written >= ROTATE_BYTES {
            self.rotate();
        }
        self.enforce_cap();
    }

    /// Перевернуть текущий лог: tray.log -> tray.1.log -> ... -> отбросить хвост.
    fn rotate(&mut self) {
        // Сдвигаем номера на единицу, начиная с конца, чтобы не затереть файлы.
        for i in (1..KEEP_FILES).rev() {
            let from = self.dir.join(format!("tray.{i}.log"));
            let to = self.dir.join(format!("tray.{}.log", i + 1));
            if from.exists() {
                let _ = std::fs::rename(&from, &to);
            }
        }
        let _ = std::fs::rename(&self.file, self.dir.join("tray.1.log"));
        self.written = 0;
    }

    /// Удалить самые старые логи, пока суммарный объём не уложится в лимит.
    fn enforce_cap(&self) {
        let Ok(mut files) = self.list_files() else { return };
        let mut total: u64 = files.iter().map(|f| f.size).sum();
        if total <= MAX_TOTAL_BYTES {
            return;
        }
        // Порядок — ПО ВРЕМЕНИ ИЗМЕНЕНИЯ, а не по размеру: по требованию
        // первыми уходят самые старые логи. Сортировка по размеру удаляла бы
        // мелкие свежие файлы и оставляла старые крупные.
        files.sort_by_key(|f| f.modified);
        for f in files {
            if total <= MAX_TOTAL_BYTES {
                break;
            }
            // Текущий лог не трогаем: иначе логирование молча сломается.
            if f.path.file_name().map(|n| n == "tray.log").unwrap_or(false) {
                continue;
            }
            if std::fs::remove_file(&f.path).is_ok() {
                total = total.saturating_sub(f.size);
            }
        }
    }

    /// Все файлы логов: путь, размер, время изменения.
    fn list_files(&self) -> std::io::Result<Vec<LogFile>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(&self.dir)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with("tray") || !name.ends_with(".log") {
                continue;
            }
            let md = e.metadata().ok();
            out.push(LogFile {
                size: md.as_ref().map(|m| m.len()).unwrap_or(0),
                // Файлы без доступного времени считаем самыми старыми.
                modified: md
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                path: e.path(),
            });
        }
        Ok(out)
    }
}

/// Метка времени: локальное время, секунды достаточно.
fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

/// Свести сообщение в одну строку: переводы строк ломают построчный разбор.
fn single_line(msg: &str) -> String {
    msg.replace('\r', " ").replace('\n', " ")
}

/// Подключить логирование. Вызывается один раз при старте.
///
/// `base` — каталог программы. Логи пишутся рядом с программой, а не в
/// системный `%TEMP%`: их нужно читать после сбоя, и они относятся к
/// установке.
pub fn init(base: &Path) {
    let logger = Logger::new(base.join("logs"));
    if let Ok(mut s) = STATE.lock() {
        *s = logger;
    }
}

/// Записать запись в лог. Безопасно вызывается откуда угодно.
pub fn log(level: &str, msg: &str) {
    // Ошибки логирования не должны приводить к панике: блокировку берём
    // осторожно, вложенные вызовы игнорируем.
    if let Ok(mut s) = STATE.lock() {
        if let Some(l) = s.as_mut() {
            l.write(level, msg);
        }
    }
}

/// Обычная запись.
pub fn info(msg: &str) {
    log("INFO", msg);
}

/// Предупреждение.
pub fn warn(msg: &str) {
    log("WARN", msg);
}

/// Ошибка.
pub fn err(msg: &str) {
    log("ERROR", msg);
}

/// Ошибка с контекстом: сообщение плюс описание ошибки.
pub fn err_ctx(what: &str, e: &dyn std::fmt::Display) {
    log("ERROR", &format!("{what}: {e}"));
}

/// Суммарный объём папки логов в байтах.
pub fn total_bytes(base: &Path) -> u64 {
    let dir = base.join("logs");
    let Ok(entries) = std::fs::read_dir(&dir) else { return 0 };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// Сколько файлов логов на диске.
pub fn file_count(base: &Path) -> usize {
    std::fs::read_dir(base.join("logs"))
        .map(|d| d.filter_map(|e| e.ok()).count())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "log/tests.rs"]
mod tests;
