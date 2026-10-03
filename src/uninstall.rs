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

/// Файл с пометкой последней выгрузки: по нему деактиватор понимает,
/// сохранял ли человек таблицу сессий.
pub const LAST_EXPORT: &str = "last_export.txt";

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

/// Проверки безопасности: лежит ли файл вне папки программы?
///
/// Выгрузку надо сохранить ДО удаления, иначе она уйдёт вместе с папкой.
fn is_outside(path: &Path, base: &Path) -> bool {
    // Разрешаем путь по папке, а не по самому файлу: файл выгрузки в этот
    // момент обычно ещё не создан, и `canonicalize` на нём падал бы. Из-за
    // этого проверка раньше отвечала «внутри папки» на совершенно внешний
    // путь и срывала выгрузку.
    let parent = path.parent().unwrap_or(base);
    let real_parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
    let real_file = match path.file_name() {
        Some(n) => real_parent.join(n),
        None => real_parent.clone(),
    };
    let real_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    !real_file.starts_with(&real_base)
}

/// Сколько строк данных в выгруженном CSV (без строки заголовка).
///
/// Нужно для отчёта: человек должен видеть, что файл не пустой. Если файл
/// прочитать не удалось, возвращаем `None` — это не повод прерывать
/// удаление.
fn csv_rows(path: &Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    Some(lines.saturating_sub(1))
}

/// Найти файл, в который программа последний раз выгружала таблицу.
pub fn last_export_path(base: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(base.join(LAST_EXPORT)).ok()?;
    let p = text.trim();
    if p.is_empty() {
        return None;
    }
    let pb = PathBuf::from(p);
    pb.is_file().then_some(pb)
}

/// Что делать с таблицей сессий перед удалением.
#[derive(Debug, PartialEq)]
pub enum ExportPlan {
    /// Человек уже выгружал таблицу — её можно удалять, путь известен.
    AlreadySaved(PathBuf),
    /// Не выгружал: нужно предложить сохранить сейчас.
    OfferToSave,
    /// БД недоступна — таблицу выгрузить нечем, продолжаем без неё.
    NoDatabase,
    /// База есть, но сессий в ней ещё не было. Это не ошибка: пустую
    /// таблицу выгружать не о чем, и человеку не нужно читать про ошибку.
    NoSessions,
}

/// Решить, что делать с таблицей, и при необходимости выгрузить её.
///
/// Требование: если таблица уже сохранена — удаляем её и говорим, где она
/// лежала. Если не сохранена — предлагаем сохранить сейчас и открываем
/// Проводник. Файл выгрузки в любом случае не должен остаться внутри папки
/// программы, откуда его снесёт удаление.
/// Куда выгружать по умолчанию, если вызывающий не указал путь.
pub fn prepare_export_default(base: &Path) -> Result<ExportPlan, String> {
    let out = crate::export::default_export_path();
    prepare_export(base, &out)
}

/// То же, но с явным путём выгрузки.
///
/// Путь передаётся извне не для красоты: во-первых, тесты не должны писать
/// на Рабочий стол, во-вторых, место под выгрузку решает вызывающий —
/// ему виднее, куда человек ожидает файл.
///
/// Проводник здесь НЕ открывается намеренно: раньше был флаг `auto`,
/// который открывал его внутри, а затем `uninstall_to` открывал его ещё
/// раз — Проводник всплывал дважды при каждом удалении, а при прогоне
/// тестов — прямо посреди `cargo test`. Единственное место открытия —
/// ветка `OfferToSave` в `uninstall_to`.
pub fn prepare_export(base: &Path, out: &Path) -> Result<ExportPlan, String> {
    if let Some(p) = last_export_path(base) {
        return Ok(ExportPlan::AlreadySaved(p));
    }
    // БД лежит рядом с программой.
    let db = base.join("sessions.db");
    if !db.is_file() {
        return Ok(ExportPlan::NoDatabase);
    }
    // Проверяем ДО выгрузки: если путь внутри папки программы, сносить его
    // потом будет уже нечем — файл снесётся вместе с программой.
    if !is_outside(out, base) {
        return Err(format!(
            "выгрузка получилась внутри папки программы ({}), её снесло бы удалением",
            out.display()
        ));
    }
    match export_sessions(&db, out) {
        // Ноль строк — база пустая, а не сломанная. Разные исходы, чтобы
        // человеку не показывали ошибку там, где всё в порядке.
        Ok(0) => return Ok(ExportPlan::NoSessions),
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    Ok(ExportPlan::OfferToSave)
}

/// Выгрузить сводку сессий из `db` в `out`.
pub fn export_sessions(db: &Path, out: &Path) -> Result<usize, String> {
    // Начало игровых суток берём из настроек, если они есть: от него
    // зависит разбивка по дням.
    let day_start = std::fs::read_to_string(db.with_file_name("config.json"))
        .ok()
        .and_then(|t| {
            serde_json::from_str::<serde_json::Value>(&t)
                .ok()
                .and_then(|v| v.get("day_start_hour").and_then(|h| h.as_u64()))
        })
        .unwrap_or(3) as u32;
    let handle = crate::db::Db::open(db, day_start).map_err(|e| format!("БД не открылась: {e}"))?;
    // AggRow не содержит числа сессий, поэтому берём его отдельно: без него
    // столбец «Сессий» в CSV был бы пустым.
    let agg = handle.aggregated();
    let rows: Vec<(String, i64, String, i64, i64)> = agg
        .iter()
        .map(|a| {
            let sessions = handle.sessions_by_game(&a.game_name).len() as i64;
            (
                a.game_name.clone(),
                a.total_duration,
                a.last_start.clone(),
                a.runs,
                sessions,
            )
        })
        .collect();
    // Пустая база — не ошибка, а ноль строк: вызывающий сам разберётся,
    // как это показать человеку.
    if rows.is_empty() {
        crate::log::info("в базе нет ни одной сессии — выгружать нечего");
        return Ok(0);
    }
    let built = crate::export::build_rows(&rows);
    let n = crate::export::write_csv(out, &built)?;
    crate::log::info(&format!("выгружено {n} строк в {}", out.display()));
    // Помечаем, что выгрузка была: повторный запуск деактиватора скажет,
    // что таблица уже сохранена, и не станет предлагать заново.
    if let Some(dir) = db.parent() {
        let _ = std::fs::write(dir.join(LAST_EXPORT), out.to_string_lossy().as_bytes());
    }
    Ok(n)
}

/// Итог удаления: что убрано, что осталось.
#[derive(Debug, Default, PartialEq)]
pub struct UninstallReport {
    pub removed: Vec<String>,
    pub missing: Vec<String>,
    pub failed: Vec<String>,
    pub kept: Vec<String>,
    /// Куда выгружена таблица сессий, если выгружали.
    pub exported_to: Option<PathBuf>,
    /// Что было с ранее сохранённой таблицей: путь, куда она убрана.
    pub previous_export: Option<PathBuf>,
    /// Текст для показа человеку (путь выгрузки, где её искать).
    pub notes: Vec<String>,
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
    uninstall_to(base, None)
}

/// То же, но с явным местом для выгрузки таблицы.
///
/// `out` — папка, куда положить CSV. Без неё берётся Рабочий стол.
///
/// Место указывается извне не только для удобства: тесты и ручные проверки
/// не должны писать на диск C:, а Рабочий стол там и находится. Плюс
/// человеку полезно самому решить, куда уйдёт копия истории.
pub fn uninstall_to(base: &Path, out: Option<&Path>) -> UninstallReport {
    let mut report = UninstallReport::default();
    crate::log::info(&format!(
        "деактивация запущена, каталог: {}",
        base.display()
    ));

    // 1. Таблица сессий. Если человек её уже выгружал — удаляем копию и
    //    говорим, где она была. Если не выгружал — предлагаем сохранить.
    let plan = match out {
        Some(dir) => {
            let path = crate::export::unique_path(dir);
            prepare_export(base, &path)
        }
        None => prepare_export_default(base),
    };
    match plan {
        Ok(ExportPlan::AlreadySaved(p)) => {
            crate::log::info(&format!("таблица уже была сохранена: {}", p.display()));
            if p.is_file() {
                if is_outside(&p, base) {
                    // Файл снаружи — самое частое и самое желанное: таблица
                    // остаётся у человека после удаления программы.
                    report.previous_export = Some(p.clone());
                    report.notes.push(format!(
                        "Ранее сохранённая таблица сессий: {}. Программа удалена, \
                         файл остался на месте.",
                        p.display()
                    ));
                } else {
                    // Файл внутри папки программы. Раньше он молча оставался
                    // сиротой: ни в одном списке целей его не было, и после
                    // деактивации в папке валялась выгрузка без программы.
                    // Убираем явно и говорим об этом — молчать нельзя, человек
                    // должен понимать, что таблица ушла вместе с программой.
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.display().to_string());
                    match std::fs::remove_file(&p) {
                        Ok(()) => report.removed.push(name),
                        Err(e) => report.failed.push(format!("{name}: {e}")),
                    }
                    report.notes.push(format!(
                        "Таблица сессий была сохранена внутрь папки программы ({}) \
                         и удалена вместе с ней. Данные потеряны.",
                        p.display()
                    ));
                }
            }
        }
        Ok(ExportPlan::OfferToSave) => {
            if let Ok(marker) = std::fs::read_to_string(base.join(LAST_EXPORT)) {
                let p = PathBuf::from(marker.trim());
                // Строки считаем по файлу, а не берём из маркера: в маркере
                // лежит путь, и раньше в сообщение вместо числа строк
                // печатался весь путь целиком.
                let rows = csv_rows(&p);
                report.exported_to = Some(p.clone());
                report.notes.push(match rows {
                    Some(n) => format!(
                        "Таблица сессий выгружена перед удалением (строк данных: {n}): {}",
                        p.display()
                    ),
                    None => format!("Таблица сессий выгружена перед удалением: {}", p.display()),
                });
                // Проводник открываем, чтобы человек сразу увидел файл.
                let _ = crate::export::reveal_in_explorer(&p);
            }
        }
        Ok(ExportPlan::NoDatabase) => {
            report.notes.push("Базы сессий не было — выгружать нечего.".to_string());
        }
        Ok(ExportPlan::NoSessions) => {
            // Пустая база — сообщаем спокойно, без слова «ошибка».
            report
                .notes
                .push("Игровых сессий ещё не было — выгружать нечего.".to_string());
        }
        Err(e) => {
            crate::log::err(&format!("выгрузка не удалась: {e}"));
            report
                .notes
                .push(format!("Выгрузить таблицу не получилось: {e}"));
        }
    }

    for f in PROGRAM_FILES.iter().chain(DATA_FILES.iter()) {
        // Себя пропускаем: файл запущен.
        if *f == "_uninstall.exe" {
            report.kept.push((*f).to_string());
            continue;
        }
        // Пометку о выгрузке тоже убираем — она больше не нужна.
        if *f == LAST_EXPORT {
            continue;
        }
        remove_one(&base.join(f), &mut report);
    }
    for d in DIRS {
        remove_one(&base.join(d), &mut report);
    }
    // Пометка о последней выгрузке.
    remove_one(&base.join(LAST_EXPORT), &mut report);
    // Отчёт о том, что намеренно оставлено.
    for (name, why) in KEEP {
        if base.join(name).exists() {
            report.kept.push((*name).to_string());
            crate::log::info(&format!("оставлено {name} ({why})"));
        }
    }
    crate::log::info(&format!("итог: {}", report.summary()));
    for n in &report.notes {
        crate::log::info(n);
    }
    // Самоудаление: иначе _uninstall.exe остался бы в папке навсегда.
    // Выполняется здесь, а не в `run`, чтобы `uninstall` можно было
    // вызывать из тестов, не трогая реальный файл.
    if !report.failed.is_empty() {
        crate::log::err("удаление выполнено не полностью");
    } else if let Ok(exe) = std::env::current_exe() {
        if exe.file_name().map(|n| n == UNINSTALLER).unwrap_or(false) {
            if let Err(e) = self_delete(&exe) {
                crate::log::err(&format!("не удалось запланировать самоудаление: {e}"));
            } else {
                crate::log::info("самоудаление запланировано");
            }
        }
    }
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

/// Имя файла деактиватора. Программа узнаёт себя по нему при запуске.
pub const UNINSTALLER: &str = "_uninstall.exe";

/// Запущен ли этот файл как деактиватор.
///
/// Проверяется имя исполняемого файла: `_uninstall.exe` — деактиватор,
/// `TraySession.exe` — программа. Отдельного бинарника нет, см. `main`.
pub fn is_uninstaller() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .map(|n| n.to_ascii_lowercase().starts_with("_uninstall"))
        .unwrap_or(false)
}

/// Ключ, которым задают папку для выгрузки таблицы сессий.
pub const EXPORT_FLAG: &str = "--export-to";

/// Папка из `--export-to`, если флаг задан.
///
/// Читается как есть: путь может быть ещё не существующим, его создаст
/// `export::write_csv`. Несуществующий путь — не ошибка, а обычное дело
/// для папки, которую человек указал впервые.
pub fn export_dir(args: &[String]) -> PathBuf {
    export_dir_opt(args).unwrap_or_else(crate::export::default_export_dir)
}

/// То же, но без умолчания: `None`, если флаг не задан.
pub fn export_dir_opt(args: &[String]) -> Option<PathBuf> {
    let i = args.iter().position(|a| a == EXPORT_FLAG)?;
    args.get(i + 1).map(PathBuf::from)
}

/// Куда попадёт файл выгрузки с учётом `--export-to` (для сообщения).
pub fn export_target(args: &[String]) -> PathBuf {
    crate::export::unique_path(&export_dir(args))
}

fn confirmation_text(base: &Path, args: &[String]) -> String {
    let mut text = format!(
        "Удаление Tray Session\n\nПапка программы: {}\n\nБудут удалены история сессий и настройки.",
        base.display()
    );
    match last_export_path(base) {
        Some(path) => text.push_str(&format!(
            "\n\nТаблица сессий уже сохранена и останется здесь:\n{}",
            path.display()
        )),
        None => text.push_str(&format!(
            "\n\nПеред удалением таблица сессий будет сохранена сюда:\n{}",
            export_target(args).display()
        )),
    }
    text.push_str("\n\nПродолжить?");
    text
}

#[cfg(all(windows, not(debug_assertions)))]
fn show_uninstaller_message(text: &str, caption: &str, flags: u32) -> i32 {
    use winapi::um::winuser::MessageBoxW;
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), flags) }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn show_uninstaller_message(text: &str, caption: &str, _flags: u32) -> i32 {
    println!("{caption}\n{text}");
    0
}

fn confirm_uninstall(base: &Path, args: &[String]) -> bool {
    let text = confirmation_text(base, args);
    #[cfg(all(windows, not(debug_assertions)))]
    {
        const MB_YESNO: u32 = 0x0000_0004;
        const MB_ICONWARNING: u32 = 0x0000_0030;
        const MB_DEFBUTTON2: u32 = 0x0000_0100;
        const IDYES: i32 = 6;
        return show_uninstaller_message(
            &text,
            "Удаление Tray Session",
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES;
    }
    #[cfg(not(all(windows, not(debug_assertions))))]
    {
        use std::io::Write;
        print!("{text}\nВведите y для подтверждения: ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .is_ok_and(|_| matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes" | "д" | "да"))
    }
}

fn report_text(report: &UninstallReport) -> String {
    let mut lines = vec![report.summary()];
    lines.extend(report.notes.iter().cloned());
    if !report.failed.is_empty() {
        lines.push(format!("Не удалось удалить: {}", report.failed.join(", ")));
    }
    if !report.kept.is_empty() {
        lines.push(format!("Остались: {}", report.kept.join(", ")));
    }
    lines.join("\n\n")
}

/// Точка входа деактиватора. Вынесена, чтобы её мог вызвать и `main`,
/// и тесты.
pub fn run(args: &[String]) -> i32 {
    let base = base_dir();

    let quiet = args.iter().any(|a| a == "--yes" || a == "-y");
    // --list: показать, что будет удалено, ничего не трогая.
    let list = args.iter().any(|a| a == "--list");

    if list {
        let mut lines = vec!["Будут удалены:".to_string()];
        for f in PROGRAM_FILES.iter().chain(DATA_FILES.iter()) {
            if *f == "_uninstall.exe" {
                lines.push(format!("  {f} (самоудаление в конце)"));
                continue;
            }
            let mark = if base.join(f).exists() { "есть" } else { "нет " };
            lines.push(format!("  [{mark}] {f}"));
        }
        for d in DIRS {
            let mark = if base.join(d).exists() { "есть" } else { "нет " };
            lines.push(format!("  [{mark}] {d}/"));
        }
        lines.push("Будет сохранено:".to_string());
        for (n, why) in KEEP {
            if base.join(n).exists() {
                lines.push(format!("  {n} — {why}"));
            }
        }
        show_uninstaller_message(&lines.join("\n"), "Tray Session — список удаления", 0);
        return 0;
    }

    if !quiet {
        if !confirm_uninstall(&base, args) {
            println!("Отменено пользователем.");
            crate::log::info("удаление отменено пользователем");
            return 1;
        }
    }

    let report = uninstall_to(&base, Some(&export_dir(args)));
    let caption = if report.failed.is_empty() {
        "Удаление Tray Session завершено"
    } else {
        "Удаление завершилось с ошибками"
    };
    show_uninstaller_message(&report_text(&report), caption, 0);
    if report.failed.is_empty() { 0 } else { 7 }
}

#[cfg(test)]
#[path = "uninstall/tests.rs"]
mod tests;
