//! Тесты деактиватора.
//!
//! Главное, что проверяется: удаляются ВСЕ файлы программы и ВСЕ
//! пользовательские данные, и при этом ничего лишнего не пропадает.

use super::*;

/// Каталог для теста — внутри проекта, на его диске.
fn sandbox(tag: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(format!("uninst_{tag}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("не создался каталог теста");
    p
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
