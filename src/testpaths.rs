//! Тестовые каталоги программы.
//!
//! Правила, которые тут зафиксированы:
//!
//! 1. Тесты не пишут никуда, кроме папки `temp_test` рядом с программой.
//!    Ни системного `%TEMP%` (это диск C:), ни рабочего каталога. Правило
//!    владельца проекта: никаких тестов и фоновых сборок на C:.
//! 2. После прогона папка вычищается. Иначе накапливаются копии баз и
//!    выгрузок, и через пару прогонов нельзя понять, какой файл от какого
//!    теста — «задвоение данных» чинит только одно: отсутствие мусора.
//!
//! Папка `temp_test` лежит рядом с программой, а не внутри `target`:
//! `target` можно снести целиком, и тогда не остаётся ничего, что можно
//! было бы случайно запустить. Всё, что внутри `temp_test`, — мусор по
//! определению и удаляется без сожалений.
//!
//! Путь считается от каталога проекта (`CARGO_MANIFEST_DIR`), а не от
//! рабочего каталога процесса: тесты идут параллельно, и относительный
//! путь увел бы их в разные места.

use std::path::{Path, PathBuf};

/// Имя папки с тестовыми данными.
pub const TEST_DIR: &str = "temp_test";

/// Метка, которой строка теста разрешает себе читать системный %TEMP%.
///
/// Читать можно — запрещено писать. Некоторые проверки обязаны сравнить
/// путь с %TEMP%, чтобы доказать, что программа туда НЕ пишет. Такие
/// строки помечаются прямо в коде, рядом с местом.
pub const ALLOW_READ_MARK: &str = "testpaths:allow-system-temp-read";

/// Корень тестовых каталогов: `<проект>/temp_test`.
///
/// Создаётся на лету — после прогона его удаляют, и следующему прогону
/// не на чем споткнуться.
pub fn test_root() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(TEST_DIR);
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Выдать каталог тесту и убрать его после прогона.
///
/// Возвращать `String` вместо `&str`, чтобы имя каталога жило ровно столько,
/// сколько нужно тесту, и не висело общим изменяемым значением.
pub fn scratch(tag: &str) -> String {
    // Имя включает поток и метку времени: параллельные прогоны не должны
    // схлопнуться в одну папку и портить друг другу данные.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = test_root().join(format!("{tag}_{}_{}", std::process::id(), stamp));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("не создался тестовый каталог");
    p.to_string_lossy().to_string()
}

/// Каталог теста, который убирается сам — в том числе при панике.
///
/// Обычный `remove_dir_all` в конце теста не срабатывает, если тест упал
/// на `assert!`, и мусор копится. Здесь за удаление отвечает `Drop`,
/// который вызывается и при раскрутке стека.
pub struct Scratch(pub PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Выдать каталог тесту с автоудалением.
pub fn scratch_guard(tag: &str) -> Scratch {
    Scratch(PathBuf::from(scratch(tag)))
}

/// Запрещённые для тестов места.
///
/// Возвращается список тех, куда писать нельзя. Используется тестом,
/// который стереженёт за собой: он ловит момент, когда кто-то снова
/// напишет в системную папку.
pub fn forbidden_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    // Системный %TEMP% — почти всегда на C:.
    v.push(std::env::temp_dir());
    if let Ok(p) = std::env::var("USERPROFILE") {
        v.push(PathBuf::from(p));
    }
    v.push(PathBuf::from("C:\\"));
    v.retain(|p| p.is_absolute());
    // Свою же папку оставлять нельзя — иначе проверка ничего не значит.
    v.retain(|p| !p.starts_with(test_root()));
    v
}

/// Находится ли путь в запрещённом месте.
pub fn is_forbidden(path: &Path) -> Option<PathBuf> {
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    forbidden_roots()
        .into_iter()
        .find(|f| p.starts_with(f) || p.to_string_lossy().to_ascii_lowercase().starts_with(&f.to_string_lossy().to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тексты всех файлов с тестами этого крейта.
    fn test_sources() -> Vec<(String, String)> {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut out = Vec::new();
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                let is_tests = p
                    .file_name()
                    .map(|n| n.to_string_lossy().ends_with("tests.rs"))
                    .unwrap_or(false);
                if p.is_dir() {
                    stack.push(p);
                } else if is_tests {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        out.push((p.to_string_lossy().to_string(), t));
                    }
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn test_code_never_uses_the_system_temp() {
        // Главный сторож. `std::env::temp_dir()` — это %TEMP%, а он на
        // диске C:, который трогать нельзя. Один такой вызов уже был:
        // тест шорткатов писал туда свой файл. Проверяется исходный текст,
        // а не результат прогона: иначе проверка зависела бы от того, в
        // каком порядке идут тесты, и ловила бы сама себя.
        let mut found = Vec::new();
        for (path, text) in test_sources() {
            for (i, line) in text.lines().enumerate() {
                // Явное разрешение на чтение: помечается прямо в строке.
                // Так исключение видно рядом с местом, а не прячется в
                // стороже — иначе через полгода его не найти и снимут
                // наугад, вместе с настоящей проверкой.
                if line.contains(ALLOW_READ_MARK) {
                    continue;
                }
                let code = line.split("//").next().unwrap_or("");
                if code.contains("temp_dir()") || code.contains("env::var(\"TEMP\")") {
                    found.push(format!("{}:{}", path, i + 1));
                }
            }
        }
        assert!(
            found.is_empty(),
            "тесты пишут в системный %TEMP% (диск C:), запрещено: {:?}. \
             Бери каталог через crate::testpaths::scratch()",
            found
        );
    }

    #[test]
    fn test_code_only_uses_temp_test_for_scratch_dirs() {
        // Мусор после прогона — тоже нарушение: он копится, и через
        // несколько прогонов нельзя понять, что от какого теста. Тесты
        // не должны строить пути напрямую, только через `testpaths`.
        let mut found = Vec::new();
        for (path, text) in test_sources() {
            for (i, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if code.contains("CARGO_MANIFEST_DIR") && code.contains("join(") {
                    found.push(format!("{}:{}", path, i + 1));
                }
            }
        }
        // Прямые пути допустимы только в самом `testpaths`.
        let real: Vec<String> = found
            .into_iter()
            .filter(|f| !f.replace('\\', "/").contains("/src/testpaths.rs"))
            .collect();
        assert!(
            real.is_empty(),
            "тесты строят пути напрямую вместо crate::testpaths: {real:?}"
        );
    }

    #[test]
    fn root_is_beside_the_project_not_on_c() {
        let r = test_root();
        assert!(r.is_dir(), "корень тестов не создался: {}", r.display());
        // Ключевое требование владельца проекта: тесты не пишут на C:.
        let low = r.to_string_lossy().to_ascii_lowercase();
        assert!(
            !low.starts_with("c:\\"),
            "тесты пишут на диск C:, это запрещено: {low}"
        );
        assert!(
            r.starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
            "тесты должны быть рядом с проектом: {}",
            r.display()
        );
    }

    #[test]
    fn system_temp_is_recognised_as_forbidden() {
        // Проверка самого запрета: иначе тест-сторож никого не поймает.
        assert!(
            is_forbidden(&std::env::temp_dir().join("что-нибудь")).is_some(),
            "системный %TEMP% не распознан как запрещённый"
        );
        assert!(
            is_forbidden(&test_root().join("внутри")).is_none(),
            "своя папка test_root принята за запрещённую"
        );
    }

    #[test]
    fn scratch_dir_is_created_and_inside_root() {
        let d = PathBuf::from(scratch("selfcheck"));
        assert!(d.is_dir(), "каталог не создался: {}", d.display());
        assert!(d.starts_with(test_root()), "каталог вне test_root: {}", d.display());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn scratch_names_do_not_collide_between_parallel_tests() {
        // Два параллельных вызова должны дать разные папки, иначе тесты
        // будут затирать файлы друг другу.
        let a = scratch("collide");
        let b = scratch("collide");
        assert_ne!(a, b, "имена тестовых папок совпали");
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }
}
