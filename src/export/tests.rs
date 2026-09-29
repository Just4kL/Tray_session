//! Тесты модуля выгрузки таблицы сессий.

use super::*;
use std::path::PathBuf;

/// Каталог для теста — `temp_test` рядом с программой (см. `testpaths`).
fn tmp(tag: &str) -> PathBuf {
    PathBuf::from(crate::testpaths::scratch(&format!("export_{tag}")))
}

#[test]
fn duration_formatting() {
    assert_eq!(format_duration(0), "0:00");
    assert_eq!(format_duration(61), "1:01");
    assert_eq!(format_duration(3661), "1:01:01");
    assert_eq!(format_duration(-90), "-1:30");
}

#[test]
fn build_rows_computes_percent_of_total() {
    // Проценты считаются от СУММЫ всех игр, а не от каждой отдельно:
    // иначе столбец «% от общего» врёт.
    let rows = vec![
        ("A".to_string(), 300i64, String::new(), 3i64, 3i64),
        ("B".to_string(), 100, String::new(), 1, 1),
    ];
    let built = build_rows(&rows);
    assert_eq!(built.len(), 2);
    assert!((built[0].percent - 75.0).abs() < 0.01, "{}", built[0].percent);
    assert!((built[1].percent - 25.0).abs() < 0.01, "{}", built[1].percent);
    assert_eq!(built[0].last, "—", "пустая дата должна давать прочерк");
}

#[test]
fn build_rows_handles_empty_total() {
    // Нулевое общее время не должно давать деление на ноль.
    let rows = vec![("A".to_string(), 0i64, String::new(), 0i64, 0i64)];
    let built = build_rows(&rows);
    assert_eq!(built[0].percent, 0.0);
}

#[test]
fn write_csv_has_header_and_rows() {
    let dir = tmp("csv");
    let out = dir.join("sessions.csv");
    let built = build_rows(&[
        ("Игра Один".to_string(), 3661i64, String::new(), 4i64, 4i64),
        ("Игра Два".to_string(), 60, String::new(), 1, 1),
    ]);
    let n = write_csv(&out, &built).expect("CSV не записан");
    assert_eq!(n, 2);
    let text = std::fs::read_to_string(&out).unwrap();
    // Шапка обязательна: без неё столбцы не понять.
    assert!(text.starts_with("№,Игра,Общее время"), "нет шапки: {text}");
    assert!(text.contains("Игра Один"), "нет первой игры");
    assert!(text.contains("1:01:01"), "нет отформатированного времени");
    // 3661 из (3661+60) = 98.39% от общего.
    assert!(text.contains("98.39%"), "проценты не посчитаны: {text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unique_path_does_not_overwrite_previous_export() {
    // При деактивации выгрузка не должна затирать прошлую таблицу:
    // человек удалил программу не для того, чтобы потерять историю.
    let dir = tmp("unique");
    let a = unique_path(&dir);
    assert_eq!(a.file_name().unwrap(), "sessions.csv");
    std::fs::write(&a, b"OLD").unwrap();
    let b = unique_path(&dir);
    assert_ne!(a, b, "прошлая выгрузка затёрта тем же именем");
    std::fs::write(&b, b"NEW").unwrap();
    let c = unique_path(&dir);
    assert!(!c.exists() && c != a && c != b, "третье имя не найдено: {c:?}");
    // Содержимое прошлых файлов цело.
    assert_eq!(std::fs::read(&a).unwrap(), b"OLD");
    assert_eq!(std::fs::read(&b).unwrap(), b"NEW");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn default_export_goes_outside_program_dir() {
    // Ключевое требование: выгрузка не должна попасть в папку программы,
    // иначе удаление снесёт её вместе с программой.
    let p = default_export_path();
    let s = p.to_string_lossy().to_ascii_lowercase();
    assert!(
        s.contains("\\desktop\\") || s.contains("\\temp\\"),
        "неожиданный путь выгрузки: {p:?}"
    );
    // Имя может быть sessions.csv, а может sessions_2.csv — если первое
    // имя уже занято прошлой выгрузкой. Затирать её нельзя.
    let name = p.file_name().unwrap().to_string_lossy().to_string();
    assert!(
        name == "sessions.csv" || (name.starts_with("sessions_") && name.ends_with(".csv")),
        "имя выгрузки неожиданно: {p:?}"
    );
    assert!(!p.exists(), "дефолтный путь уже занят — должно выбираться свободное");
}

#[test]
fn write_csv_creates_missing_parent_dirs() {
    // Отдельные папки для выгрузки создаются сами: человек не должен
    // заранее готовить каталог.
    let dir = tmp("mkparent");
    let out = dir.join("глубоко").join("вложенно").join("sessions.csv");
    let built = build_rows(&[("A".to_string(), 60i64, String::new(), 1i64, 1i64)]);
    assert_eq!(write_csv(&out, &built).expect("не создалась папка"), 1);
    assert!(out.is_file(), "файл не записан");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_csv_reports_unwritable_target() {
    // Ошибка должна возвращаться текстом, а не паникой: деактиватор
    // продолжает удаление даже если выгрузка не удалась.
    // Недоступное место готовим честно: кладём ФАЙЛ туда, где нужна папка.
    let dir = tmp("bad");
    let blocker = dir.join("занято");
    std::fs::write(&blocker, b"this-is-a-file-not-a-dir").unwrap();
    let out = blocker.join("sessions.csv");
    let built = build_rows(&[("A".to_string(), 60i64, String::new(), 1i64, 1i64)]);
    let e = write_csv(&out, &built).unwrap_err();
    assert!(!e.is_empty(), "ошибка без описания");
    let _ = std::fs::remove_dir_all(&dir);
}
