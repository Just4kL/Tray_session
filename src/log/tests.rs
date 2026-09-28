//! Тесты логирования. Проверяется то, что логи не могут занять диск:
//! лимит суммарного объёма и удаление самых старых файлов.

use super::*;

/// Каталог для тестов — внутри проекта, на его диске (см. замечание в
/// `update::tests` про %TEMP% на C:).
fn test_dir(tag: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(format!("log_{tag}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("не создался каталог теста");
    p
}

#[test]
fn writes_and_reads_back() {
    let dir = test_dir("basic");
    // Логируем напрямую в файл, не трогая глобальное состояние.
    let l = Logger::new(dir.join("logs")).expect("логгер не создался");
    let mut l = l;
    l.write("INFO", "первая строка");
    l.write("ERROR", "вторая строка");
    let text = std::fs::read_to_string(dir.join("logs").join("tray.log")).unwrap();
    assert!(text.contains("первая строка"), "{text}");
    assert!(text.contains("[ERROR]"), "нет уровня записи: {text}");
    // Многострочное сообщение не должно рвать строку в файле.
    assert_eq!(text.lines().count(), 2, "сообщение разбилось на строки: {text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn multiline_message_stays_on_one_line() {
    let dir = test_dir("multiline");
    let mut l = Logger::new(dir.join("logs")).unwrap();
    l.write("WARN", "строка1\nстрока2\r\nстрока3");
    let text = std::fs::read_to_string(dir.join("logs").join("tray.log")).unwrap();
    assert_eq!(text.lines().count(), 1, "перенос не убран: {text}");
    assert!(text.contains("строка1") && text.contains("строка3"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rotates_when_file_too_big() {
    let dir = test_dir("rotate");
    let mut l = Logger::new(dir.join("logs")).unwrap();
    // Пишем заметно больше порога вращения.
    let chunk = "x".repeat(4096);
    for _ in 0..(ROTATE_BYTES as usize / 4096 + 8) {
        l.write("INFO", &chunk);
    }
    let logs = dir.join("logs");
    assert!(logs.join("tray.log").exists(), "текущий лог пропал");
    assert!(
        logs.join("tray.1.log").exists(),
        "лог не перевернулся: файлы {:?}",
        std::fs::read_dir(&logs).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect::<Vec<_>>()
    );
    // Текущий файл после вращения не превышает порог.
    let cur = std::fs::metadata(logs.join("tray.log")).unwrap().len();
    assert!(cur < ROTATE_BYTES * 2, "текущий лог не уменьшился: {cur}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn total_size_never_exceeds_cap() {
    // ГЛАВНОЕ требование: сколько бы логов ни написали, папка не должна
    // превысить лимит. Заполняем намного больше 10 МБ и проверяем.
    let dir = test_dir("cap");
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs).unwrap();

    // Создаём файлы от самых старых к самым новым, каждый заметно больше
    // порога вращения, чтобы сумма точно перевалила за 10 МБ.
    let big = ROTATE_BYTES + 64 * 1024;
    for i in 0..30u32 {
        let name = if i == 0 { "tray.log".to_string() } else { format!("tray.{i}.log") };
        std::fs::write(logs.join(name), vec![b'a'; big as usize]).unwrap();
    }
    let before: u64 = std::fs::read_dir(&logs)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();
    assert!(
        before > MAX_TOTAL_BYTES,
        "подготовка неудачна: {before} байт, должно быть больше {MAX_TOTAL_BYTES}"
    );

    // Один вызов записи запускает проверку лимита.
    let mut l = Logger::new(logs.clone()).unwrap();
    l.write("INFO", "проверка лимита");

    let after: u64 = std::fs::read_dir(&logs)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();
    assert!(
        after <= MAX_TOTAL_BYTES,
        "лимит превышен после уборки: {after} > {MAX_TOTAL_BYTES}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cap_removes_oldest_first() {
    // Под лимит должны уходить самые старые файлы, а свежий tray.log
    // обязан уцелеть: иначе логирование молча перестанет работать.
    let dir = test_dir("oldest");
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    let big = ROTATE_BYTES + 64 * 1024;
    // Текущий лог делаем маленьким, иначе он превысит порог вращения и
    // уедет в tray.1.log при первой же записи — это вращение, а не уборка,
    // и проверять тут нечего.
    std::fs::write(logs.join("tray.log"), b"fresh-current-log-entry").unwrap();
    for i in 1..30u32 {
        std::fs::write(logs.join(format!("tray.{i}.log")), vec![b'a'; big as usize]).unwrap();
    }
    // Разводим файлы по времени: чем больше номер, тем свежее, поэтому
    // первым под нож должен попасть tray.1.log.
    let now = std::time::SystemTime::now();
    for i in 1..30u32 {
        let p = logs.join(format!("tray.{i}.log"));
        let t = now - std::time::Duration::from_secs((30 - i) as u64 * 60);
        let _ = filetime_set(&p, t);
    }
    let _ = filetime_set(&logs.join("tray.log"), now);

    let mut l = Logger::new(logs.clone()).unwrap();
    l.write("INFO", "уборка");

    assert!(logs.join("tray.log").exists(), "текущий лог удалён — это поломка");
    assert!(
        !logs.join("tray.1.log").exists(),
        "самый старый файл не удалён"
    );
    assert!(
        logs.join("tray.29.log").exists(),
        "свежий файл удалён вместо старого"
    );
    let total: u64 = std::fs::read_dir(&logs)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();
    assert!(total <= MAX_TOTAL_BYTES, "лимит превышен: {total}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Проставить файлу время изменения. Только для тестов: в основном коде
/// этого делать не нужно.
fn filetime_set(p: &Path, t: std::time::SystemTime) -> std::io::Result<()> {
    let f = std::fs::OpenOptions::new().write(true).open(p)?;
    f.set_modified(t)?;
    Ok(())
}

#[test]
fn logging_never_panics_on_unwritable_dir() {
    // Программа может лежать в Program Files без прав на запись. Тогда
    // логирование обязано молча отключиться, а не ронять программу.
    // Подменяем глобальное состояние на логгер с недоступной папкой.
    let before = std::panic::catch_unwind(|| {
        let l = Logger::new(PathBuf::from("Z:\\definitely\\not\\writable\\logs"));
        match l {
            Some(mut l) => l.write("INFO", "попытка записи"),
            None => { /* логгер не создался — тоже не паника */ }
        }
    });
    assert!(before.is_ok(), "логирование упало на недоступной папке");
}

#[test]
fn global_api_is_safe_without_init() {
    // info/warn/err можно звать до init() и после сброса — паник быть не должно.
    let ok = std::panic::catch_unwind(|| {
        info("до init");
        warn("до init");
        err("до init");
        err_ctx("контекст", &"ошибка");
    });
    assert!(ok.is_ok(), "глобальное логирование упало без init()");
}
