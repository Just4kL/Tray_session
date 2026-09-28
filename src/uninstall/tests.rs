//! Тесты деактиватора.
//!
//! Главное, что проверяется: удаляются ВСЕ файлы программы и ВСЕ
//! пользовательские данные, и при этом ничего лишнего не пропадает.

use super::*;

/// Каталог для теста — внутри проекта, на его диске.
///
/// Удаляется сам при выходе из теста, даже если тест упал с паникой: иначе
/// в `target/test-tmp` копится мусор, а диск C:/F: человек трогать не должен.
struct Sandbox(PathBuf);

impl std::ops::Deref for Sandbox {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Sandbox {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Создать каталог для теста.
fn sandbox(tag: &str) -> Sandbox {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(format!("uninst_{tag}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("не создался каталог теста");
    Sandbox(p)
}

/// Разложить в каталоге правдоподобную установку: файлы программы,
/// пользовательские данные, логи и то, что удалять нельзя.
fn make_install(dir: &Path) {
    for f in PROGRAM_FILES {
        if *f == "_uninstall.exe" {
            continue;
        }
        std::fs::write(dir.join(f), b"PAYLOAD").unwrap();
    }
    for f in DATA_FILES {
        std::fs::write(dir.join(f), b"USER-DATA").unwrap();
    }
    std::fs::create_dir_all(dir.join("logs")).unwrap();
    std::fs::write(dir.join("logs").join("tray.log"), b"[INFO] log\n").unwrap();
    std::fs::create_dir_all(dir.join("update_tmp")).unwrap();
    std::fs::write(dir.join("update_tmp").join("payload.exe"), b"X").unwrap();
    // То, что обязано уцелеть.
    std::fs::write(dir.join("README.md"), b"INSTRUCTIONS").unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src").join("app.rs"), b"CODE").unwrap();
}

#[test]
fn removes_every_program_and_data_file() {
    let dir = sandbox("all");
    make_install(&dir);
    let report = uninstall(&dir);

    assert!(report.is_clean(), "есть неудачные удаления: {:?}", report.failed);
    // Ни одного файла программы и данных не осталось.
    for f in PROGRAM_FILES.iter().chain(DATA_FILES.iter()) {
        if *f == "_uninstall.exe" {
            continue;
        }
        assert!(
            !dir.join(f).exists(),
            "файл {f} не удалён — пользовательские данные должны уходить"
        );
    }
    // Папки тоже.
    for d in DIRS {
        assert!(!dir.join(d).exists(), "папка {d} не удалена");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_instructions_and_sources() {
    let dir = sandbox("keep");
    make_install(&dir);
    uninstall(&dir);
    // Инструкция и исходники обязаны уцелеть: после удаления программы
    // README негде прочитать, а исходники — не артефакт установки.
    assert!(dir.join("README.md").exists(), "README удалён — нельзя");
    assert!(dir.join("src").exists(), "исходники удалены — нельзя");
    assert_eq!(
        std::fs::read(dir.join("README.md")).unwrap(),
        b"INSTRUCTIONS",
        "содержимое README изменилось"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn removes_logs_folder_with_its_contents() {
    // Папка логов должна сноситься ПОД конец, а не оставаться с хвостом.
    //
    // Здесь намеренно НЕ вызывается `log::init`: логгер — глобальное
    // состояние, общее для параллельных тестов, и вызов `init` из одного
    // теста перенаправлял записи чужого теста в его каталог. Из-за этого
    // проверка падала в общем прогоне и проходила в одиночку. Поведение
    // «деактиватор пишет лог» проверяется сквозным прогоном в отдельной
    // папке, а не здесь.
    let dir = sandbox("logs");
    make_install(&dir);
    let report = uninstall(&dir);
    assert!(report.is_clean(), "ошибки удаления: {:?}", report.failed);
    assert!(
        !dir.join("logs").exists(),
        "папка логов осталась: {:?}",
        report
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_files_are_not_errors() {
    // Повторный запуск на полупустом каталоге не должен считаться ошибкой:
    // отсутствующее удалять нечего.
    let dir = sandbox("missing");
    let report = uninstall(&dir);
    assert!(report.is_clean(), "пустой каталог дал ошибки: {:?}", report.failed);
    assert!(report.removed.is_empty(), "что-то «удалилось» из пустоты");
    assert!(!report.missing.is_empty(), "отсутствующие не отмечены");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn uninstaller_does_not_delete_itself_directly() {
    // Запущенный файл удалить нельзя: деактиватор обязан пропустить себя и
    // положиться на самоудаление через cmd. Иначе он всегда будет падать с
    // ошибкой «занято».
    let dir = sandbox("self");
    make_install(&dir);
    std::fs::write(dir.join("_uninstall.exe"), b"RUNNING").unwrap();
    let report = uninstall(&dir);
    assert!(
        dir.join("_uninstall.exe").exists(),
        "деактиватор удалил себя напрямую — это не сработает на Windows"
    );
    assert!(
        report.kept.contains(&"_uninstall.exe".to_string()),
        "себя не отметил как оставленный: {:?}",
        report.kept
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saved_table_is_reported_not_deleted() {
    // Вариант 1: человек УЖЕ сохранял таблицу. Деактиватор должен сказать,
    // где она лежит, и оставить файл на месте.
    let dir = sandbox("saved");
    make_install(&dir);
    // Выгрузка лежит СНАРУЖИ папки программы - так и должно быть.
    let exported = dir.join("..").join(format!("выгрузка_{}.csv", std::process::id()));
    std::fs::write(&exported, "№,Игра\n1,A\n").unwrap();
    std::fs::write(dir.join(LAST_EXPORT), exported.to_string_lossy().as_bytes()).unwrap();

    let report = uninstall(&dir);
    assert!(report.is_clean(), "ошибки удаления: {:?}", report.failed);
    // Путь назван человеку.
    let notes = report.notes.join(" ");
    assert!(notes.contains("sessions") || notes.contains(exported.to_string_lossy().as_ref()),
        "в отчёте нет пути к сохранённой таблице: {notes:?}");
    assert!(
        exported.is_file(),
        "сохранённая таблица исчезла: {}",
        exported.display()
    );
    let _ = std::fs::remove_file(&exported);
}

#[test]
fn export_is_written_outside_the_folder_being_deleted() {
    // Вариант 2: таблицу не сохраняли. Ключевое требование — файл выгрузки
    // НЕ должен остаться в папке, которую сейчас снесут, иначе история
    // исчезнет вместе с программой.
    let dir = sandbox("notsaved");
    make_install(&dir);
    // Настоящая база вместо заглушки: make_install положил туда «USER-DATA».
    std::fs::remove_file(dir.join("sessions.db")).unwrap();
    add_fake_sessions(&dir);

    // Выгрузка целиком мимо песочницы - так же, как в жизни (Рабочий стол).
    // Путь задаём явно, чтобы тест не писал на настоящий Рабочий стол.
    let out_dir = dir.join("..").join("экспорт_снаружи");
    let _ = std::fs::create_dir_all(&out_dir);
    let outside = out_dir.join("sessions.csv");
    let plan = prepare_export(&dir, &outside, false);
    match &plan {
        Ok(ExportPlan::OfferToSave) => {}
        other => panic!("ожидалось предложение сохранить, получено {other:?}"),
    }
    // Проверяем, что записанный файл лежит вне папки удаления.
    let marker = std::fs::read_to_string(dir.join(LAST_EXPORT))
        .expect("не записана пометка о выгрузке");
    let written = PathBuf::from(marker.trim());
    assert!(written.is_file(), "выгрузка не создана: {}", written.display());
    assert!(
        is_outside(&written, &dir),
        "выгрузка попала ВНУТРЬ удаляемой папки: {}",
        written.display()
    );
    // И главное: сама деактивация не должна унести её с собой.
    let report = uninstall(&dir);
    assert!(report.is_clean(), "ошибки удаления: {:?}", report.failed);
    assert!(
        written.is_file(),
        "выгрузка исчезла вместе с папкой программы: {}",
        written.display()
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn previous_export_inside_program_dir_is_reported_as_lost() {
    // Если выгрузка всё-таки оказалась внутри папки программы, её снесёт
    // вместе с программой. Молчать об этом нельзя - человек должен узнать,
    // что данные потеряны.
    let dir = sandbox("inside");
    make_install(&dir);
    let inside = dir.join("sessions.csv");
    std::fs::write(&inside, "x").unwrap();
    std::fs::write(dir.join(LAST_EXPORT), inside.to_string_lossy().as_bytes()).unwrap();

    let report = uninstall(&dir);
    let notes = report.notes.join(" ");
    assert!(
        notes.contains("потеряна") || notes.contains("удалена вместе"),
        "не сказано, что выгрузка внутри папки потеряна: {notes:?}"
    );
    assert!(!inside.exists(), "файл внутри папки не удалён");
}

#[test]
fn empty_database_does_not_block_uninstall() {
    // Пустая база - не повод останавливать удаление: просто говорим, что
    // выгружать нечего.
    let dir = sandbox("emptydb");
    make_install(&dir);
    // Настоящая, но ПУСТАЯ база: заглушка из make_install не открывается.
    std::fs::remove_file(dir.join("sessions.db")).unwrap();
    crate::db::Db::open(dir.join("sessions.db"), 3).expect("БД не создалась");
    let report = uninstall(&dir);
    assert!(report.is_clean(), "удаление застряло: {:?}", report.failed);
    // Пустая база — не ошибка: в отчёте не должно быть слова «ошибка».
    let notes = report.notes.join(" ");
    assert!(notes.contains("нечего"), "{:?}", report.notes);
    assert!(!notes.contains("ошибк"), "пустая база названа ошибкой: {notes:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn last_export_marker_is_removed_with_the_program() {
    // Метка `last_export.txt` нужна только пока программа жива. После
    // деактивации она не должна валяться в папке сиротой.
    let dir = sandbox("marker");
    make_install(&dir);
    std::fs::write(dir.join(LAST_EXPORT), r"C:\Users\X\Desktop\sessions.csv").unwrap();
    let report = uninstall(&dir);
    assert!(report.is_clean(), "{:?}", report.failed);
    assert!(!dir.join(LAST_EXPORT).exists(), "метка о выгрузке осталась");
}

#[test]
fn stale_marker_does_not_block_fresh_export() {
    // Метка может остаться с прошлой установки, а самого файла уже нет
    // (человен его перенёс или удалил). Тогда это не «уже сохранено» —
    // нужно выгрузить заново, иначе история уйдёт с программой молча.
    let dir = sandbox("stale");
    make_install(&dir);
    std::fs::remove_file(dir.join("sessions.db")).unwrap();
    add_fake_sessions(&dir);
    // Указываем на НЕСУЩЕСТВУЮЩИЙ файл: метка устаревшая.
    std::fs::write(
        dir.join(LAST_EXPORT),
        r"C:\Users\X\Desktop\missing-export.csv",
    )
    .unwrap();

    let out_dir = dir.join("..").join("экспорт_свежий");
    let _ = std::fs::create_dir_all(&out_dir);
    let out = out_dir.join("sessions.csv");
    let plan = prepare_export(&dir, &out, false);
    assert!(
        matches!(plan, Ok(ExportPlan::OfferToSave)),
        "при устаревшей метке нужно выгружать заново, получено {plan:?}"
    );
    assert!(out.is_file(), "свежая выгрузка не записана: {}", out.display());
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn export_refuses_to_write_inside_the_program_folder() {
    // Ключевое требование: файл не должен остаться в папке, которую снесут.
    // Проверяем, что мы это ловим ДО записи, а не обнаруживаем потом.
    let dir = sandbox("refuse");
    make_install(&dir);
    std::fs::remove_file(dir.join("sessions.db")).unwrap();
    add_fake_sessions(&dir);

    let inside = dir.join("sessions.csv");
    let plan = prepare_export(&dir, &inside, false);
    assert!(
        matches!(plan, Err(_)),
        "выгрузка внутрь папки программы должна отклоняться, получено {plan:?}"
    );
    assert!(!inside.exists(), "файл всё равно записался внутрь папки");
}

#[test]
fn mode_is_detected_by_file_name() {
    // Программа и деактиватор — один файл, различаются именем.
    // Проверяем правило, а не результат `current_exe` (он привязан к
    // cargo-тесту, а не к `_uninstall.exe`).
    assert!(UNINSTALLER.eq_ignore_ascii_case("_uninstall.exe"));
    assert!(!UNINSTALLER.eq_ignore_ascii_case("traysession.exe"));
    // Регистр не должен мешать: Windows имена не различает.
    assert!("_UNINSTALL.EXE".to_ascii_lowercase().starts_with("_uninstall"));
}

/// Собрать настоящую тестовую установку в `test1` для ручной проверки.
///
/// Отдельный игнорируемый тест, а не скрипт: база сессий пишется тем же
/// кодом, что и у программы. Скрипт не смог бы создать её без дублирования
/// SQL, и через полгода схема изменилась бы, а скрипт остался бы старым.
///
/// Запуск: `cargo test prepare_test1_install -- --ignored`
#[test]
#[ignore = "готовит папку test1 для ручной проверки, не запускать в обычном прогоне"]
fn prepare_test1_install() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("test1");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("не создалась test1");

    // Настоящая сборка, иначе проверять нечего.
    let exe = root.join("target").join("release").join("game-session-tracker.exe");
    assert!(exe.is_file(), "Сначала собери release: {}", exe.display());
    for name in ["TraySession.exe", "_uninstall.exe"] {
        std::fs::copy(&exe, dir.join(name)).expect("не скопирован исполняемый файл");
    }
    std::fs::copy(root.join("README.md"), dir.join("README.md")).ok();

    // Настройки и прочие данные — чтобы деактиватору было что удалять.
    std::fs::write(
        dir.join("config.json"),
        r#"{"day_start_hour":3,"update_freq":"weekly","update_auto":true,"update_channel":"beta"}"#,
    )
    .unwrap();
    std::fs::write(dir.join("known_games.json"), "[]").unwrap();
    std::fs::write(dir.join("shortcuts.json"), "{}").unwrap();
    std::fs::write(dir.join("update_manifest.json"), "[]").unwrap();
    std::fs::create_dir_all(dir.join("logs")).unwrap();
    std::fs::write(dir.join("logs").join("tray.log"), "[INFO] test install\n").unwrap();
    std::fs::create_dir_all(dir.join("update_tmp")).unwrap();
    std::fs::write(dir.join("update_tmp").join("payload.exe"), "X").unwrap();

    // Живая база с сессиями: без неё деактиватор скажет «выгружать нечего».
    add_fake_sessions(&dir);

    println!("Готово: {}", dir.display());
    for e in std::fs::read_dir(&dir).unwrap() {
        println!("  {}", e.unwrap().file_name().to_string_lossy());
    }
}

/// Собрать `test2` — папку, где человек вручную проверяет удаление и
/// обновление на ПРЕДЫДУЩЕЙ версии.
///
/// Отличия от `test1`, и каждое существенное:
/// * кладётся старая сборка `TraySession.exe` из корня репозитория, а не
///   свежая — иначе обновлять нечего, программа сразу была бы новой;
/// * в `config.json` прописан канал `beta` — иначе обновление пойдёт в
///   стабильную ветку, где лежит другая (старая) сборка, и человек увидит
///   «обновлений нет» вместо проверки;
/// * ставится метка `last_export.txt` с НЕСУЩЕСТВУЮЩИМ файлом: так
///   проверяется первый вариант деактивации — «таблица ещё не выгружалась».
///
/// Запуск: `cargo test prepare_test2_install -- --ignored --nocapture`
#[test]
#[ignore = "готовит папку test2 для ручной проверки, не запускать в обычном прогоне"]
fn prepare_test2_install() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("test2");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("не создалась test2");

    // Именно ПРЕДЫДУЩАЯ сборка — та, чей хеш был в манифесте до бампа.
    // Копию делает tools\make-manifest.ps1 -Archive при пересборке,
    // поэтому она не теряется при замене файла в корне на новый.
    let old_dir = root.join("dist").join("old");
    let old = std::fs::read_dir(&old_dir)
        .ok()
        .and_then(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|e| e == "exe").unwrap_or(false))
                .min_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
        })
        .unwrap_or_else(|| panic!(
            "Нет прошлой сборки в {}. Собери её так: перезапусти \
             `tools\\make-manifest.ps1 -Archive` после `cargo build --release`.",
            old_dir.display()
        ));
    std::fs::copy(&old, dir.join("TraySession.exe")).expect("не скопирована старая сборка");
    // А деактиватор — свежий, из release-сборки. Старая версия не умеет
    // режима деактивации по имени файла: запустила бы обычное окно
    // программы вместо удаления, и проверять было бы нечего. После
    // обновления оба файла станут одной версии — как в настоящей
    // установке.
    let fresh = root.join("target").join("release").join("game-session-tracker.exe");
    assert!(fresh.is_file(), "Сначала собери release: {}", fresh.display());
    std::fs::copy(&fresh, dir.join("_uninstall.exe")).expect("не скопирован деактиватор");
    std::fs::copy(root.join("README.md"), dir.join("README.md")).ok();

    // Канал beta — обязателен, см. описание теста.
    std::fs::write(
        dir.join("config.json"),
        r#"{"day_start_hour":3,"update_freq":"weekly","update_auto":false,"update_channel":"beta"}"#,
    )
    .unwrap();
    std::fs::write(dir.join("known_games.json"), "[]").unwrap();
    std::fs::write(dir.join("shortcuts.json"), "{}").unwrap();
    // Пустой манифест: программа не должна доверять файлу на диске, она
    // качает манифест из репозитория. Здесь он нужен лишь как файл-заглушка.
    std::fs::write(dir.join("update_manifest.json"), "[]").unwrap();
    std::fs::create_dir_all(dir.join("logs")).unwrap();
    std::fs::write(dir.join("logs").join("tray.log"), "[INFO] test2 install\n").unwrap();
    std::fs::create_dir_all(dir.join("update_tmp")).unwrap();
    // Метка на несуществующий файл: деактиватор должен решить, что
    // таблица ещё не выгружалась, и предложить сохранить её.
    std::fs::write(dir.join(LAST_EXPORT), r"C:\Users\Test\Desktop\old-export.csv").unwrap();
    add_fake_sessions(&dir);

    println!("Готово: {}", dir.display());
    println!("Версия в test2: предыдущая сборка, канал обновлений: beta");
    for e in std::fs::read_dir(&dir).unwrap() {
        println!("  {}", e.unwrap().file_name().to_string_lossy());
    }
}

/// Наполнить базу сессиями за несколько дней, чтобы выгрузка была
/// не пустой и выглядела как настоящая таблица.
fn add_fake_sessions(base: &Path) {
    use chrono::Local;
    let db = crate::db::Db::open(base.join("sessions.db"), 3).expect("БД не открылась");
    let now = Local::now();
    // Разные игры и разная длительность — иначе не проверить, что
    // проценты и суммы считаются по-разному.
    let games = [
        ("Cyberpunk 2077", 4i64),
        ("Rust", 2),
        ("Elden Ring", 7),
    ];
    for (name, hours) in games {
        // По сессии в день за последние трое суток.
        for days_back in 0..3 {
            let start = now - chrono::Duration::days(days_back) - chrono::Duration::hours(hours + 4);
            if let Ok(id) = db.add_session(name, "game.exe", &start) {
                let _ = db.end_session(id, &(start + chrono::Duration::hours(hours)));
            }
        }
    }
}

#[test]
fn list_of_targets_is_complete() {
    // Список целей не должен разъезжаться с тем, что создаёт программа.
    // Пользовательские файлы — самое важное: потеря истории сессий недопустима.
    for f in [
        "sessions.db",
        "config.json",
        "known_games.json",
        "shortcuts.json",
    ] {
        assert!(
            DATA_FILES.contains(&f),
            "{f} не в списке на удаление — данные останутся"
        );
    }
    assert!(PROGRAM_FILES.contains(&"TraySession.exe"));
    assert!(PROGRAM_FILES.contains(&"_uninstall.exe"));
    for d in ["logs", "update_tmp"] {
        assert!(DIRS.contains(&d), "{d} не в списке папок на удаление");
    }
}

#[test]
fn report_counts_are_consistent() {
    let dir = sandbox("counts");
    make_install(&dir);
    let r = uninstall(&dir);
    // Ничего не должно быть одновременно удалённым и отсутствовавшим.
    for f in &r.removed {
        assert!(!r.missing.contains(f), "{f} и удалён, и отсутствует");
    }
    assert!(r.summary().contains("удалено"), "{}", r.summary());
    let _ = std::fs::remove_dir_all(&dir);
}
