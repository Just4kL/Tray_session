//! Деактиватор Tray Session: `_uninstall.exe`.
//!
//! Удаляет всё, что создаёт программа: исполняемые файлы, манифест
//! обновлений, логи, временную папку обновлений и пользовательские данные
//! (история сессий, настройки, список игр, шорткаты).
//!
//! Что НЕ удаляется: `README.md`. Это инструкция, а не артефакт установки,
//! и после удаления программы её негде прочитать.
//!
//! Пути считаются от каталога самого деактиватора, поэтому он работает
//! независимо от того, откуда запущен. Каждый шаг пишется в лог рядом с
//! программой: после удаления логов может не остаться, но в процессе работы
//! видно, что и почему не удалилось.

use std::path::{Path, PathBuf};

/// Файлы программы, которые удаляются.
pub const PROGRAM_FILES: &[&str] = &[
    "TraySession.exe",
    "Tray_sesstion_setup.exe", // старое имя, могло остаться от прежних сборок
    "Tray_session_setup.exe",
    "_uninstall.exe",
    "update_manifest.json",
];

/// Пользовательские данные: история сессий, настройки, списки.
pub const DATA_FILES: &[&str] = &[
    "sessions.db",
    "sessions.db-journal",
    "sessions.db-wal",
    "sessions.db-shm",
    "config.json",
    "known_games.json",
    "shortcuts.json",
    "update_ready.flag",
];

/// Папки, которые удаляются целиком.
pub const DIRS: &[&str] = &["logs", "update_tmp", "dist"];

/// Что удаляем, а что оставляем, и почему.
pub const KEEP: &[(&str, &str)] = &[
    ("README.md", "инструкция, не артефакт установки"),
    ("Cargo.toml", "исходники не ставятся установщиком"),
    ("Cargo.lock", "исходники не ставятся установщиком"),
    ("src", "исходники не ставятся установщиком"),
    ("tools", "исходники не ставятся установщиком"),
    (".git", "история репозитория"),
    ("target", "каталог сборки Rust"),
];

/// Итог удаления: что убрано, что осталось.
#[derive(Debug, Default, PartialEq)]
pub struct UninstallReport {
    pub removed: Vec<String>,
    pub missing: Vec<String>,
    pub failed: Vec<String>,
    pub kept: Vec<String>,
}

impl UninstallReport {
    /// Однострочный итог для показа пользователю.
    pub fn summary(&self) -> String {
        format!(
            "удалено: {}, отсутствовало: {}, не удалось: {}",
            self.removed.len(),
            self.missing.len(),
            self.failed.len()
        )
    }
    /// Есть ли смысл считать удаление успешным.
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty()
    }
}

/// Удалить один файл или папку, записав результат.
fn remove_one(p: &Path, report: &mut UninstallReport) {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.display().to_string());
    if !p.exists() {
        report.missing.push(name);
        return;
    }
    // Папку с логами снимаем последней: до конца работы нужен открытый лог.
    let r = if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    };
    match r {
        Ok(()) => {
            crate::log::info(&format!("удалено: {name}"));
            report.removed.push(name);
        }
        Err(e) => {
            crate::log::err(&format!("НЕ удалено {name}: {e}"));
            report.failed.push(format!("{name}: {e}"));
        }
    }
}

/// Выполнить удаление в каталоге `base`.
///
/// Не удаляет сам деактиватор: запущенный файл Windows удалить не даёт.
/// Он убирает себя через `cmd /c` с задержкой (см. `self_delete`).
pub fn uninstall(base: &Path) -> UninstallReport {
    let mut report = UninstallReport::default();
    crate::log::info(&format!(
        "деактивация запущена, каталог: {}",
        base.display()
    ));

    for f in PROGRAM_FILES.iter().chain(DATA_FILES.iter()) {
        // Себя пропускаем: файл запущен.
        if *f == "_uninstall.exe" {
            report.kept.push((*f).to_string());
            continue;
        }
        remove_one(&base.join(f), &mut report);
    }
    for d in DIRS {
        remove_one(&base.join(d), &mut report);
    }
    // Отчёт о том, что намеренно оставлено.
    for (name, why) in KEEP {
        if base.join(name).exists() {
            report.kept.push((*name).to_string());
            crate::log::info(&format!("оставлено {name} ({why})"));
        }
    }
    crate::log::info(&format!("итог: {}", report.summary()));
    report
}

/// Удалить деактиватор после завершения работы.
///
/// Windows не даёт удалить исполняемый файл, пока он запущен, поэтому
/// просим `cmd` удалить его с задержкой — к тому моменту процесс уже вышел.
pub fn self_delete(exe: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW, чтобы не мигало окно консоли.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args([
            "/C",
            "ping",
            "127.0.0.1",
            "-n",
            "3",
            ">",
            "nul",
            "&",
            "del",
            "/F",
            "/Q",
        ])
        .arg(exe)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

/// Каталог, в котором лежит деактиватор.
pub fn base_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Точка входа деактиватора. Вынесена, чтобы её мог вызвать и отдельный
/// бинарник `uninstall`, и тесты.
pub fn main() {
    let base = base_dir();
    crate::log::init(&base);

    let args: Vec<String> = std::env::args().collect();
    // --yes: без вопросов. Нужно для автоматической проверки и для
    // сценария «тихо удалить». По умолчанию спрашиваем подтверждение,
    // потому что удаляется история сессий.
    let quiet = args.iter().any(|a| a == "--yes" || a == "-y");
    let dry = args.iter().any(|a| a == "--dry-run");
    // --list: показать, что будет удалено, ничего не трогая.
    let list = args.iter().any(|a| a == "--list");

    if list {
        println!("Будут удалены:");
        for f in PROGRAM_FILES.iter().chain(DATA_FILES.iter()) {
            if *f == "_uninstall.exe" {
                println!("  {f} (самоудаление в конце)");
                continue;
            }
            let mark = if base.join(f).exists() { "есть" } else { "нет " };
            println!("  [{mark}] {f}");
        }
        for d in DIRS {
            let mark = if base.join(d).exists() { "есть" } else { "нет " };
            println!("  [{mark}] {d}/");
        }
        println!("Будет сохранено:");
        for (n, why) in KEEP {
            if base.join(n).exists() {
                println!("  {n} — {why}");
            }
        }
        return;
    }

    if dry {
        let r = UninstallReport::default();
        let _ = r;
        println!("--dry-run: ничего не удалено (используйте --list для обзора)");
        return;
    }

    if !quiet {
        println!("Удаление Tray Session");
        println!("Каталог: {}", base.display());
        println!("Будет удалена история игровых сессий и все настройки.");
        print!("Удалить всё? (y/N) ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut s = String::new();
        if std::io::stdin().read_line(&mut s).is_err() {
            println!("Отмена.");
            return;
        }
        if !matches!(s.trim().to_ascii_lowercase().as_str(), "y" | "yes" | "д" | "да") {
            println!("Отменено пользователем.");
            crate::log::info("удаление отменено пользователем");
            return;
        }
    }

    let report = uninstall(&base);
    println!("{}", report.summary());
    if !report.failed.is_empty() {
        println!("Не удалось:");
        for f in &report.failed {
            println!("  {f}");
        }
    }
    if !report.kept.is_empty() {
        println!("Сохранено: {}", report.kept.join(", "));
    }

    // Самоудаление: иначе _uninstall.exe остался бы в папке навсегда.
    if let Ok(exe) = std::env::current_exe() {
        if let Err(e) = self_delete(&exe) {
            println!("Не удалось запланировать самоудаление: {e}");
            println!("Удалите вручную: {}", exe.display());
        }
    }
}

#[cfg(test)]
#[path = "uninstall/tests.rs"]
mod tests;
