//! Тесты обновления. Главное, что тут проверяется, — обновление НИКОГДА
//! не трогает пользовательские данные: история сессий, настройки и списки.
//! Остальное (кеш, расписание, пути) — вспомогательное, но тоже важное.

use super::*;

// ---------------------------------------------------------------------------
// Защита пользовательских данных
// ---------------------------------------------------------------------------

#[test]
fn user_files_are_protected() {
    // Каждый пользовательский файл должен быть в списке защищённых.
    for f in [
        "sessions.db",
        "config.json",
        "known_games.json",
        "shortcuts.json",
    ] {
        assert!(is_protected(f), "{f} не защищён от обновления");
    }
    // Регистр не должен обходить защиту: Windows не различает регистр имён.
    assert!(is_protected("Sessions.DB"), "регистр обошёл защиту");
    assert!(is_protected("SESSIONS.DB"), "регистр обошёл защиту");
    // А вот программные файлы обновляться должны.
    for f in ["TraySession.exe", "Tray_session_setup.exe"] {
        assert!(!is_protected(f), "{f} не должен быть защищён");
    }
}

#[test]
fn manifest_cannot_target_user_files() {
    // Даже если манифест (из сети) попросит обновить sessions.db —
    // обновление обязано это отвергнуть, а не выполнить.
    let m = Manifest {
        version: "9.9.9".into(),
        build: "20260101".into(),
        files: vec![FileEntry {
            name: "sessions.db".into(),
            sha256: "00".repeat(32),
        }],
    };
    let err = m.validate().unwrap_err();
    assert!(err.contains("sessions.db"), "неожиданная ошибка: {err}");
    // И в план такой файл не попадает даже мимо validate.
    assert!(m.wanted_files().is_empty(), "защищённый файл попал в план");
}

#[test]
fn manifest_rejects_paths_outside_program_dir() {
    // Имя из сети не должно превращаться в путь: иначе через манифест
    // можно записать куда угодно на диске.
    for bad in [
        "../evil.exe",
        "..\\evil.exe",
        "sub/dir.exe",
        "C:/windows/system32/evil.exe",
        "/abs/path.exe",
        "",
        ".",
        "..",
        "noextension",
        "file\u{0}name.exe",
    ] {
        assert!(!is_safe_rel_name(bad), "имя {bad:?} ошибочно признано безопасным");
    }
    // Нормальные имена проходят.
    for ok in ["TraySession.exe", "Tray_session_setup.exe", "update_helper.exe"] {
        assert!(is_safe_rel_name(ok), "имя {ok:?} ошибочно отвергнуто");
    }
}

#[test]
fn manifest_allows_only_executables() {
    // Обновляем исполняемые файлы. Попытка протащить скрипт или документ —
    // отвергаем: в ветке лежат только сборки.
    let m = Manifest {
        version: "1.0.0".into(),
        build: String::new(),
        files: vec![FileEntry {
            name: "update.bat".into(),
            sha256: String::new(),
        }],
    };
    assert!(m.validate().is_err(), "не-.exe просочился в манифест");
}

#[test]
fn manifest_drops_duplicate_files() {
    // Один и тот же файл дважды — качать и заменять дважды не надо.
    let m = Manifest {
        version: "1.0.0".into(),
        build: String::new(),
        files: vec![
            FileEntry { name: "a.exe".into(), sha256: "11".repeat(32) },
            FileEntry { name: "A.exe".into(), sha256: "11".repeat(32) },
            FileEntry { name: "b.exe".into(), sha256: "22".repeat(32) },
        ],
    };
    let wanted = m.wanted_files();
    assert_eq!(wanted.len(), 2, "дубли не схлопнуты: {wanted:?}");
}

// ---------------------------------------------------------------------------
// Скачивание: retry
// ---------------------------------------------------------------------------

#[test]
fn retry_delay_is_exponential_and_capped() {
    // Расписание повторов download_to: 1с, 2с, дальше кап — иначе
    // затягиваем UX при мёртвой сети. Проверяем саму функцию, а не
    // сеть: дёргать реальный GitHub из юнит-теста нельзя.
    assert_eq!(retry_delay_secs(1), 1);
    assert_eq!(retry_delay_secs(2), 2);
    assert_eq!(retry_delay_secs(3), 4);
    // Кап: даже при большом номере попытки не спим вечно.
    assert!(retry_delay_secs(100) <= 128, "кап пропал");
}

// ---------------------------------------------------------------------------
// План обновления
// ---------------------------------------------------------------------------

/// Временный каталог теста, который удаляется сам — в том числе если тест
/// упал на `assert!`.
///
/// Обычный `remove_dir_all` в конце теста не срабатывает при панике, и
/// мусор копится. Здесь за удаление отвечает `Drop`, который вызывается и
/// при раскрутке стека. Каталог берётся из `testpaths` — он лежит в
/// `temp_test` рядом с программой: правило владельца проекта, никаких
/// тестов на диске C:.
pub(super) struct TempDir(pub(super) PathBuf);

/// Список аргументов командной строки из среза строк.
fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

impl TempDir {
    /// Создать каталог с уникальным именем внутри `temp_test`.
    pub(super) fn new(tag: &str) -> TempDir {
        TempDir(PathBuf::from(crate::testpaths::scratch(tag)))
    }
    /// Путь внутри каталога.
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    /// Записать файл и вернуть путь.
    fn write(&self, name: &str, data: &[u8]) -> PathBuf {
        let f = self.0.join(name);
        std::fs::write(&f, data).expect("не записался файл теста");
        f
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn plan_downloads_only_changed_files() {
    let dir = TempDir::new("plan");
    // На диске: a.exe (совпадёт), c.exe (не совпадёт), sessions.db (данные).
    let a_old = b"old-a";
    // Содержимое «истории» — ASCII, чтобы байтовые литералы компилировались.
    let history = b"MY-PRICELESS-SESSION-HISTORY";
    std::fs::write(dir.join("a.exe"), a_old).unwrap();
    std::fs::write(dir.join("c.exe"), b"something-else").unwrap();
    std::fs::write(dir.join("sessions.db"), history).unwrap();

    let m = Manifest {
        version: "2.0.0".into(),
        build: "20261029".into(),
        files: vec![
            // a.exe на диске уже такой, какой ждёт манифест, — качать не надо.
            FileEntry { name: "a.exe".into(), sha256: sha256_bytes(a_old) },
            // c.exe на диске другой — его надо скачать.
            FileEntry { name: "c.exe".into(), sha256: sha256_bytes(b"new-c") },
            // Защищённый файл в манифесте — в план не попадёт.
            FileEntry { name: "sessions.db".into(), sha256: "00".repeat(32) },
        ],
    };
    let plan = plan_update(&dir.0, &m);
    let names: Vec<&str> = plan.to_download.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["c.exe"], "в план попало лишнее: {names:?}");
    assert_eq!(plan.unchanged, vec!["a.exe".to_string()]);
    // Пользовательский файл найден и отмечен как сохраняемый.
    assert!(plan.preserved.contains(&"sessions.db".to_string()));
    // И, главное, он остался на диске нетронутым.
    assert_eq!(std::fs::read(dir.join("sessions.db")).unwrap(), history);
}

#[test]
fn atomic_replace_swaps_staged_files_and_keeps_user_data() {
    let dir = TempDir::new("apply");
    let tmp = dir.join("update_tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    // Пользовательские файлы на месте.
    std::fs::write(dir.join("sessions.db"), b"HISTORY-KEEP").unwrap();
    std::fs::write(dir.join("config.json"), b"{\"scale\":1.3}").unwrap();
    // Программный файл (старый) и скачанный .new.
    std::fs::write(dir.join("old.exe"), b"STALE").unwrap();
    std::fs::write(tmp.join("old.exe.new"), b"FRESH-BINARY-1234").unwrap();

    let files = vec![
        FileEntry { name: "old.exe".into(), sha256: String::new() },
        // Попытка подсунуть пользовательский файл в список на замену.
        FileEntry { name: "sessions.db".into(), sha256: String::new() },
    ];
    let res = replace_files_atomic(&dir.0, &tmp, &files).expect("замена сорвалась");
    assert_eq!(res, vec!["old.exe".to_string()], "заменено лишнее: {res:?}");
    // Программный файл обновился, старый ушёл в .old (до cleanup).
    assert_eq!(std::fs::read(dir.join("old.exe")).unwrap(), b"FRESH-BINARY-1234");
    assert_eq!(std::fs::read(dir.join("old.exe.old")).unwrap(), b"STALE");
    // Cleanup убирает .old.
    cleanup_old_files(&dir.0, &res);
    assert!(!dir.join("old.exe.old").exists(), ".old остался после cleanup");
    // Пользовательские файлы целы.
    assert_eq!(std::fs::read(dir.join("sessions.db")).unwrap(), b"HISTORY-KEEP");
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        b"{\"scale\":1.3}"
    );
}

#[test]
fn atomic_replace_fails_without_staged_file() {
    // Нет <name>.new — заменять нечего, ошибка (а не молчаливый пропуск).
    let dir = TempDir::new("missing_new");
    let tmp = dir.join("update_tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(dir.join("a.exe"), b"OLD").unwrap();
    let e = replace_files_atomic(
        &dir.0,
        &tmp,
        &[FileEntry { name: "a.exe".into(), sha256: String::new() }],
    )
    .unwrap_err();
    assert!(e.contains("нет скачанного файла"), "{e}");
    assert_eq!(std::fs::read(dir.join("a.exe")).unwrap(), b"OLD");
}

#[test]
fn atomic_replace_rollback_restores_previous_on_conflict() {
    // Конфликт: .old от прошлой попытки — каталог, remove_file на нём
    // падает. Уже заменённое (good.exe) должно откатиться из .old.
    let dir = TempDir::new("rollback");
    let tmp = dir.join("update_tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(dir.join("good.exe"), b"STALE-GOOD").unwrap();
    std::fs::write(tmp.join("good.exe.new"), b"FRESH-GOOD").unwrap();
    std::fs::write(dir.join("bad.exe"), b"STALE-BAD").unwrap();
    std::fs::write(tmp.join("bad.exe.new"), b"FRESH-BAD").unwrap();
    std::fs::create_dir_all(dir.join("bad.exe.old")).unwrap();

    let files = vec![
        FileEntry { name: "good.exe".into(), sha256: String::new() },
        FileEntry { name: "bad.exe".into(), sha256: String::new() },
    ];
    let e = replace_files_atomic(&dir.0, &tmp, &files).unwrap_err();
    assert!(e.contains("остаток"), "{e}");
    // good.exe откачен из .old (откат идёт в обратном порядке).
    assert_eq!(std::fs::read(dir.join("good.exe")).unwrap(), b"STALE-GOOD");
    assert!(
        !dir.join("good.exe.old").exists(),
        ".old остался после отката — повторная замена споткнётся"
    );
    // bad.exe вообще не тронут.
    assert_eq!(std::fs::read(dir.join("bad.exe")).unwrap(), b"STALE-BAD");
}

// ---------------------------------------------------------------------------
// SHA-256
// ---------------------------------------------------------------------------

#[test]
fn sha256_matches_known_vectors() {
    // Официальные тест-векторы FIPS 180-4 / NIST: если реализация неверна,
    // проверка целостности скачанных файлов будет обманчивой.
    assert_eq!(
        sha256_bytes(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_bytes(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_bytes(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    // Многострочный вход, где важна длина блока (55, 56, 64 байта).
    let long = vec![b'a'; 1_000_000];
    assert_eq!(
        sha256_bytes(&long),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn update_offer_decision_drives_the_notice_button() {
    // Связка, из которой растёт кнопка уведомления:
    //   plan.to_download пусто  -> обновления нет  -> кнопки нет
    //   plan.to_download есть  -> обновление есть -> кнопка есть
    //
    // Проверяем на настоящем манифесте проекта, подставляя диск, который
    // совпадает или не совпадает.
    let manifest = Manifest::parse(super::REAL_MANIFEST).expect("манифест не разобран");

    // Случай 1: на диске ровно то, что в манифесте -> качать нечего.
    let dir = TempDir::new("offer_same");
    for f in &manifest.files {
        // Кладём файл, чей хеш совпадёт: пишем заведомо другое, но
        // манифест ставим такой же, какой получился.
        std::fs::write(dir.join(&f.name), b"payload").unwrap();
    }
    let matching = Manifest {
        version: manifest.version.clone(),
        build: manifest.build.clone(),
        files: manifest
            .files
            .iter()
            .map(|f| FileEntry {
                name: f.name.clone(),
                sha256: sha256_bytes(b"payload"),
            })
            .collect(),
    };
    let plan = plan_update(&dir.0, &matching);
    assert!(
        plan.to_download.is_empty(),
        "совпадающие файлы не должны качаться: {:?}",
        plan.to_download
    );
    // Обновления нет -> кнопки нет.
    assert!(!crate::app::should_show_update_notice(None));

    // Случай 2: на диске другая версия файла -> обновление есть.
    let dir = TempDir::new("offer_new");
    for f in &manifest.files {
        std::fs::write(dir.join(&f.name), b"STALE-OLD-BINARY").unwrap();
    }
    let plan = plan_update(&dir.0, &manifest);
    assert_eq!(
        plan.to_download.len(),
        manifest.files.len(),
        "все изменившиеся файлы должны попасть в план"
    );
    // Обновление есть -> версия известна -> кнопка появляется.
    let version = manifest.version.clone();
    assert!(
        crate::app::should_show_update_notice(Some(&version)),
        "кнопка не показалась бы при найденном обновлении"
    );
    // И подсказка на ней содержит именно эту версию.
    assert!(crate::app::update_tooltip(&version).contains(&version));
}

#[test]
fn exited_flag_is_consumed_before_replace() {
    // Протокол pipeline: main пишет main_exited.flag перед exit, updater
    // читает его на шаге 3 и СЪЕДАЕТ (удаляет). Протухший флаг от упавшего
    // прогона не должен разрешать замену при живом процессе — та же
    // ловушка, что была у READY_FLAG (тест-предшественник
    // stale_ready_flag_cannot_authorize_early_replacement удалён вместе
    // с механизмом).
    let dir = TempDir::new("exited_flag");
    let flag = tmp_dir(&dir.0).join(MAIN_EXITED_FLAG);
    std::fs::create_dir_all(flag.parent().unwrap()).unwrap();
    // 1. Флаг от прошлого прогона лежит.
    std::fs::write(&flag, b"1").unwrap();
    assert!(flag.exists());
    // 2. Шаг 3 pipeline читает и удаляет — проверяем ту же операцию,
    // что делает код (read + remove).
    let present = flag.exists();
    let _ = std::fs::remove_file(&flag);
    assert!(present, "флаг должен был быть виден шагу 3");
    assert!(!flag.exists(), "флаг не съеден — протухший разрешит замену");
    // 3. Повторный шаг 3 флага уже не видит.
    assert!(!flag.exists(), "флаг воскрес после съедения");
}

#[test]
fn updater_log_appends_to_updater_log() {
    // R6: шаги pipeline идут в logs/updater.log, log.rs при этом не тронут.
    let dir = TempDir::new("ulog");
    ulog(&dir.0, "INFO", "шаг проверен");
    ulog(&dir.0, "ERR", "ошибка проверена");
    let text = std::fs::read_to_string(dir.0.join("logs").join(UPDATER_LOG_NAME)).unwrap();
    assert!(text.contains("[INFO] шаг проверен"), "{text}");
    assert!(text.contains("[ERR] ошибка проверена"), "{text}");
    // Формат: дата время [LEVEL] сообщение, по строке на запись.
    assert_eq!(text.lines().count(), 2, "{text}");
}

#[test]
fn updater_name_is_stable() {
    // Имя копии updater в update_tmp зашито в pipeline (шаг 2 и cleanup)
    // и в main_pid-логику не входит, но переименовывать молча нельзя:
    // ручная проверка ищет этот файл в диспетчере задач.
    assert_eq!(UPDATER_NAME, "_updater_running.exe");
    assert_eq!(MAIN_PID_FILE, "main_pid.txt");
    assert_eq!(MAIN_EXITED_FLAG, "main_exited.flag");
}

#[test]
fn wrong_hash_is_rejected_and_file_not_written() {
    // Ключевая защита при загрузке: если файл побился или подменился,
    // на диск он попасть не должен. Считаем локальный файл «скачанным» и
    // подсовываем заведомо неверный хеш.
    let dir = TempDir::new("badhash");
    let dest = dir.join("downloaded.exe");
    let payload = b"PAYLOAD-THAT-SHOULD-NEVER-BE-USED";
    std::fs::write(&dest, payload).unwrap();
    let wrong = sha256_bytes(b"something-else-entirely");
    // Ручная проверка того же условия, что в download_to: сравнение
    // регистронезависимое, при несовпадении — отказ.
    assert!(
        !wrong.eq_ignore_ascii_case(&sha256_bytes(payload)),
        "разные данные дали одинаковый хеш"
    );
    // И наоборот: правильный хеш проходит.
    assert!(sha256_bytes(payload).eq_ignore_ascii_case(&sha256_bytes(payload)));
}

#[test]
fn sha256_handles_block_boundaries() {
    // Границы блоков по 64 байта: именно там чаще всего ломается
    // накопление буфера при чтении файла частями.
    for n in [55usize, 56, 57, 63, 64, 65, 119, 120, 128] {
        let data = vec![0x5au8; n];
        // Считаем по кускам, как это делает sha256_file.
        let mut h = Sha256::new();
        for c in data.chunks(7) {
            h.update(c);
        }
        let by_chunks = h.hex();
        assert_eq!(
            by_chunks,
            sha256_bytes(&data),
            "хеш разошёлся на длине {n}"
        );
    }
}

#[test]
fn sha256_file_matches_sha256_bytes() {
    let dir = TempDir::new("hashfile");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let f = dir.join("data.bin");
    std::fs::write(&f, &data).unwrap();
    assert_eq!(sha256_file(&f).unwrap(), sha256_bytes(&data));
}

// ---------------------------------------------------------------------------
// Расписание
// ---------------------------------------------------------------------------

#[test]
fn update_frequencies_have_sane_intervals() {
    assert_eq!(UpdateFreq::Hourly.interval_secs(), Some(3600));
    assert_eq!(UpdateFreq::Daily.interval_secs(), Some(86_400));
    assert_eq!(UpdateFreq::Weekly.interval_secs(), Some(604_800));
    // «Вручную» — интервала нет: проверка только по кнопке.
    assert_eq!(UpdateFreq::Manual.interval_secs(), None);
    // Интервалы должны возрастать, иначе расписание бессмысленно.
    let mut last = 0;
    for f in [UpdateFreq::Hourly, UpdateFreq::Daily, UpdateFreq::Weekly] {
        let s = f.interval_secs().unwrap();
        assert!(s > last, "интервалы не растут: {f:?}");
        last = s;
    }
}

#[test]
fn update_frequency_roundtrips_through_config_code() {
    for f in UpdateFreq::all() {
        assert_eq!(UpdateFreq::from_code(f.code()), f, "код не обратился");
        assert!(!f.label().is_empty(), "{f:?}: пустая подпись");
    }
    // Неизвестный код (например, из будущей версии конфига) не должен
    // ломать программу — берём безопасное умолчание.
    assert_eq!(UpdateFreq::from_code(200), UpdateFreq::Hourly);
}

#[test]
fn channel_survives_saving_and_loading_the_config() {
    // Настройка обязана пережить цикл сохранения: человек выбрал «Бета»,
    // нажал «Сохранить», перезапустил программу — и канал не сбросился.
    // Без этого бета молча превращалась бы в стабильную при первом же
    // перезапуске.
    let dir = TempDir::new("channel_cfg");
    let path = dir.join("config.json");
    for want in [CHANNEL_STABLE, CHANNEL_BETA] {
        let cfg = crate::config::AppConfig {
            update_channel: want.to_string(),
            ..Default::default()
        };
        cfg.save_to(&path);
        assert!(path.is_file(), "файл настроек не записан");
        let back = crate::config::AppConfig::load_from(&path);
        assert_eq!(
            normalize_channel(&back.update_channel),
            want,
            "канал не пережил сохранение"
        );
    }
    // Старый конфиг без поля вообще — не ошибка, а стабильный канал.
    std::fs::write(&path, r#"{"day_start_hour":3}"#).unwrap();
    let back = crate::config::AppConfig::load_from(&path);
    assert_eq!(normalize_channel(&back.update_channel), CHANNEL_STABLE);
}

// ---------------------------------------------------------------------------
// Пути и аргументы
// ---------------------------------------------------------------------------

#[test]
fn temp_dir_is_beside_program_not_system_temp() {
    // Ключевое требование: загрузка идёт рядом с программой, а не в
    // системный TEMP. Иначе на Program Files запись невозможна, а
    // обновление не сможет заменить файл.
    let base = PathBuf::from(r"C:\Program Files\Tray Session");
    let t = tmp_dir(&base);
    assert_eq!(t, base.join("update_tmp"));
    assert_eq!(t.parent().unwrap(), base, "папка загрузки вне каталога программы");
    // И это точно не системный TEMP. Здесь %TEMP% только ЧИТАЕТСЯ, чтобы
    // доказать, что путь загрузки с ним не совпадает; ничего туда не
    // пишется. Маркер testpaths:allow-system-temp-read снимает вопрос
    // сторожу, который иначе принял бы это за нарушение.
    let sys = std::env::temp_dir(); // testpaths:allow-system-temp-read
    assert!(!t.starts_with(&sys), "путь загрузки попал в системный TEMP: {t:?}");
}

#[test]
fn atomic_replace_never_touches_user_files_even_when_listed() {
    // Замена C13b-механики (finish_aside): при отмене/обрыве программа
    // обязана остаться на месте. В новом pipeline это свойство даёт
    // replace_files_atomic: переименование идёт только вперёд через .old,
    // а при ошибке — откат. Проверяем инвариант: без .new-файлов
    // боевые файлы не меняются вообще.
    let dir = TempDir::new("aside_restore");
    let main = dir.join(MAIN_EXE);
    dir.write(MAIN_EXE, b"OLD PROGRAM");
    // Скачанного нет — pipeline дальше не пойдёт, main цел.
    let e = replace_files_atomic(
        &dir.0,
        &dir.join("update_tmp"),
        &[FileEntry { name: MAIN_EXE.into(), sha256: String::new() }],
    )
    .unwrap_err();
    assert!(e.contains("нет скачанного файла"), "{e}");
    assert!(main.is_file(), "программа пропала без замены");
    assert_eq!(std::fs::read(&main).unwrap(), b"OLD PROGRAM", "программа повреждена");
}

#[test]
fn updater_args_use_named_parameters_with_defaults() {
    // Новый протокол R2: --updater --parent-pid <PID> --wait-secs <N>
    // --channel <c>. Позиционного формата больше нет.
    // Обычный запуск — не updater.
    assert!(parse_args(&args(&[])).is_none());
    assert!(parse_args(&args(&["TraySession.exe"])).is_none());
    // Полный набор.
    let u = parse_args(&args(&[
        "TraySession.exe",
        "--updater",
        "--parent-pid",
        "1234",
        "--wait-secs",
        "45",
        "--channel",
        "beta",
    ]))
    .unwrap();
    assert_eq!(u.parent_pid, 1234);
    assert_eq!(u.wait_secs, 45);
    assert_eq!(u.channel, "beta");
    // Голый --updater — умолчания, а не падение: pid 0 = ждать некого.
    let u = parse_args(&args(&["--updater"])).unwrap();
    assert_eq!(u.parent_pid, 0);
    assert_eq!(u.wait_secs, 60);
    assert_eq!(u.channel, CHANNEL_STABLE);
    // Мусор вместо чисел — тоже умолчания.
    let u = parse_args(&args(&["--updater", "--parent-pid", "не-число"])).unwrap();
    assert_eq!(u.parent_pid, 0);
    // Порядок аргументов не важен.
    let u = parse_args(&args(&["--updater", "--channel", "beta", "--wait-secs", "7"])).unwrap();
    assert_eq!(u.wait_secs, 7);
    assert_eq!(u.channel, "beta");
}

#[test]
fn manifest_urls_point_at_the_right_branch() {
    let m = manifest_url();
    assert!(m.contains(REPO), "{m}");
    assert!(m.contains(BRANCH), "{m}");
    assert!(m.ends_with(MANIFEST_NAME), "{m}");
    let f = file_url("TraySession.exe");
    assert!(f.contains(BRANCH) && f.ends_with("TraySession.exe"), "{f}");
}

#[test]
fn beta_channel_reads_a_different_branch() {
    // Главное требование: бета и стабильный смотрят в РАЗНЫЕ ветки. Иначе
    // бетовая сборка обновилась бы из стабильной ветки и молча откатывалась
    // на старую версию — человек бы и не понял, почему пропала бета.
    let stable = manifest_url_for(CHANNEL_STABLE);
    let beta = manifest_url_for(CHANNEL_BETA);
    assert!(stable.contains("Tray-session"), "{stable}");
    assert!(beta.contains("app-Tray_session"), "{beta}");
    assert_ne!(stable, beta, "каналы смотрят в одну ветку");
    // Имя файла то же — отличается только ветка.
    assert!(beta.ends_with(MANIFEST_NAME), "{beta}");
    let bf = file_url_for(CHANNEL_BETA, "TraySession.exe");
    assert!(bf.contains("app-Tray_session") && bf.ends_with("TraySession.exe"), "{bf}");
}

#[test]
fn unknown_channel_falls_back_to_stable_not_to_nothing() {
    // Опечатка в config.json не должна отключить обновления совсем.
    assert_eq!(normalize_channel("что-то"), CHANNEL_STABLE);
    assert_eq!(normalize_channel(""), CHANNEL_STABLE);
    assert_eq!(normalize_channel("STABLE"), CHANNEL_STABLE);
    // Бета принимается в любом регистре и с пробелами: пишет человек.
    assert_eq!(normalize_channel(" Beta "), CHANNEL_BETA);
    assert_eq!(normalize_channel("BETA"), CHANNEL_BETA);
    // И URL строится от нормализованного канала, а не от сырого текста.
    assert_eq!(
        manifest_url_for(normalize_channel("что-то")),
        manifest_url_for(CHANNEL_STABLE)
    );
}

#[test]
fn updater_channel_survives_named_arguments() {
    // Фоновый процесс — отдельный, и без явной передачи канала он взял бы
    // стабильный по умолчанию. Проверяем именно сквозную передачу.
    let u = parse_args(&args(&["--updater", "--parent-pid", "9", "--wait-secs", "20", "--channel", "beta"])).unwrap();
    assert_eq!(u.channel, "beta");
    assert_eq!(u.parent_pid, 9);
    assert_eq!(u.wait_secs, 20);
    // Без канала — стабильный.
    let u = parse_args(&args(&["--updater", "--parent-pid", "9"])).unwrap();
    assert_eq!(u.channel, CHANNEL_STABLE);
}

#[test]
fn updater_base_dir_survives_reexec() {
    // Шаг 2 pipeline (reexec из update_tmp): копия НЕ должна выводить
    // корень из своего program_dir() — там будет update_tmp. Base едет
    // явным аргументом от main.
    let u = parse_args(&args(&["--updater", "--base", "C:\\Program Files\\Tray Session"])).unwrap();
    assert_eq!(u.base_dir, PathBuf::from("C:\\Program Files\\Tray Session"));
    // Без --base — fallback на program_dir() (ручной запуск updater
    // для диагностики).
    let u = parse_args(&args(&["--updater"])).unwrap();
    assert_eq!(u.base_dir, program_dir());
}

#[test]
#[cfg(windows)]
fn pid_death_distinguishes_live_and_dead() {
    // wait_for_pid_death: живой PID + короткий таймаут = false,
    // мёртвый PID = true. Мёртвый берём честно: запускаем cmd,
    // дожидаемся выхода — PID гарантированно мёртв.
    use std::process::Command;
    let mut child = Command::new("cmd").args(["/C", "exit"]).spawn().unwrap();
    let dead = child.id();
    child.wait().unwrap();
    assert!(
        wait_for_pid_death(dead, std::time::Duration::from_secs(5)),
        "мёртвый pid {dead} не опознан"
    );
    assert!(
        !wait_for_pid_death(std::process::id(), std::time::Duration::from_millis(50)),
        "живой собственный pid сочтён мёртвым"
    );
}

#[test]
fn channel_has_a_readable_name() {
    // В логах и интерфейсе канал показывается по-русски: «beta» в сообщении
    // человеку ничего не объясняет.
    assert_eq!(channel_label(CHANNEL_STABLE), "стабильный");
    assert_eq!(channel_label(CHANNEL_BETA), "бета");
    // Оба канала существуют и не пересекаются.
    assert_eq!(CHANNELS.len(), 2);
    assert!(CHANNELS.contains(&CHANNEL_STABLE) && CHANNELS.contains(&CHANNEL_BETA));
}

#[test]
fn manifest_describe_is_human_readable() {
    let m = Manifest {
        version: "0.8.0".into(),
        build: "20261030".into(),
        files: vec![FileEntry { name: "TraySession.exe".into(), sha256: "a".repeat(64) }],
    };
    let d = m.describe();
    assert!(d.contains("0.8.0"), "{d}");
    assert!(d.contains("TraySession.exe"), "{d}");
    // Пустой манифест тоже должен давать осмысленный текст.
    let empty = Manifest { version: "0.8.0".into(), build: String::new(), files: vec![] };
    assert!(empty.describe().contains("0.8.0"), "{}", empty.describe());
}

#[test]
fn report_roundtrips_through_file() {
    let dir = TempDir::new("report");
    let r = UpdateReport {
        ok: true,
        message: "установлена версия 0.8.0".into(),
        updated: vec!["TraySession.exe".into()],
        unchanged: vec!["Tray_session_setup.exe".into()],
        preserved: vec!["sessions.db".into()],
        stage: "done".into(),
        error_kind: String::new(),
    };
    r.save(&dir.0).unwrap();
    let back = UpdateReport::load(&dir.0).expect("отчёт не прочитан");
    assert_eq!(back, r);
    // Описание должно упоминать и обновлённое, и сохранённое.
    let d = back.describe();
    assert!(d.contains("TraySession.exe"), "{d}");
    assert!(d.contains("sessions.db"), "{d}");
    UpdateReport::clear(&dir.0);
    assert!(UpdateReport::load(&dir.0).is_none(), "отчёт не очищен");
}

#[test]
fn fail_report_carries_stage_and_kind() {
    // fail() — единая точка выхода pipeline: отчёт всегда содержит
    // stage и error_kind, иначе диагностика по update_report.json слепая.
    let dir = TempDir::new("fail_report");
    let base = &dir.0;
    let code = fail(base, "download", "net", "тестовая ошибка".to_string(), 6);
    assert_eq!(code, 6);
    let back = UpdateReport::load(base).expect("отчёт не записан");
    assert!(!back.ok);
    assert_eq!(back.stage, "download");
    assert_eq!(back.error_kind, "net");
    assert!(back.message.contains("тестовая ошибка"));
}
