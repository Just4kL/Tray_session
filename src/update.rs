//! Обновление программы с GitHub.
//!
//! Устройство: обновление НИКОГДА не трогает пользовательские данные.
//! Программа забирает с GitHub манифест, где перечислено, какие файлы
//! изменились, и заменяет только их. История сессий (`sessions.db`),
//! настройки (`config.json`, `shortcuts.json`, `known_games.json`)
//! в список обновляемых не входят и при несовпадении версий остаются
//! нетронутыми.
//!
//! Загрузка идёт отдельным процессом — тем же самым `.exe` с флагом
//! `--updater` (см. `main.rs`). Так обновление не блокирует интерфейс и
//! может перезапустить программу после замены файлов. Качаем во временную
//! папку РЯДОМ с программой (`<каталог программы>/update_tmp`), а не в
//! системный `%TEMP%`: на Program Files туда писать нельзя, и рядом с
//! программой всегда есть права на запись.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Константы
// ---------------------------------------------------------------------------

/// Репозиторий, откуда берутся сборки и манифест.
pub const REPO: &str = "Just4kL/Tray_session";
/// Канал обновлений: стабильный.
pub const CHANNEL_STABLE: &str = "stable";
/// Канал обновлений: бета, для новых функций.
pub const CHANNEL_BETA: &str = "beta";

/// Ветка, в которой лежат сборки по каналу.
///
/// Раньше ветка была одна и жёстко вшита, из-за чего сборка с новой
/// функцией молча уходила на стабильную: человек ставил бету, а программа
/// возвращала его на старую. Теперь ветку выбирает настройка.
pub fn branch_for(channel: &str) -> &'static str {
    if channel.eq_ignore_ascii_case(CHANNEL_BETA) {
        "app-Tray_session"
    } else {
        // Любое неизвестное значение считаем стабильным: не должно быть
        // так, чтобы опечатка в настройке оставила человека без
        // обновлений вовсе.
        "Tray-session"
    }
}

/// Ветка по умолчанию, если канал не задан.
pub const BRANCH: &str = "Tray-session";

/// Название канала по-русски, для сообщений человеку.
pub fn channel_label(channel: &str) -> &'static str {
    if channel.eq_ignore_ascii_case(CHANNEL_BETA) {
        "бета"
    } else {
        "стабильный"
    }
}

/// Все допустимые каналы — для интерфейса и для проверки настроек.
pub const CHANNELS: &[&str] = &[CHANNEL_STABLE, CHANNEL_BETA];

/// Привести строку настройки к каналу: неизвестное значение молча
/// становится стабильным, чтобы опечатка не отключила обновления.
pub fn normalize_channel(raw: &str) -> &'static str {
    if raw.trim().eq_ignore_ascii_case(CHANNEL_BETA) {
        CHANNEL_BETA
    } else {
        CHANNEL_STABLE
    }
}
/// Имя файла манифеста в репозитории: список изменившихся файлов и версия.
pub const MANIFEST_NAME: &str = "update_manifest.json";
/// Папка для загрузки — рядом с программой, не в системном TEMP.
pub const TMP_DIR_NAME: &str = "update_tmp";
/// Файл с результатом фонового процесса; основной процесс читает его,
/// чтобы показать пользователю итог обновления.
pub const REPORT_NAME: &str = "update_report.json";
/// Имя главного исполняемого файла программы.
pub const MAIN_EXE: &str = "TraySession.exe";
/// Имя файла с PID главного процесса. Main пишет его при старте;
/// updater и --updated-перезапуск читают его, чтобы дождаться
/// смерти main или завершить её (C5-mutex иначе блокирует рестарт).
pub const MAIN_PID_FILE: &str = "main_pid.txt";
/// Флаг «main вышел сам»: main создаёт его перед process::exit(0)
/// при старте обновления. Updater использует как fallback-подтверждение
/// готовности к замене (на случай, если PID переиспользован ОС).
pub const MAIN_EXITED_FLAG: &str = "main_exited.flag";

// ---------------------------------------------------------------------------
// Что нельзя трогать
// ---------------------------------------------------------------------------

/// Файлы пользователя: история сессий, настройки, списки. Обновление их
/// никогда не удаляет и не перезаписывает, даже если их имена случайно
/// встретятся в манифесте — защита от «обновление снесло мою историю».
pub const PROTECTED_FILES: &[&str] = &[
    "sessions.db",
    "sessions.db-journal",
    "sessions.db-wal",
    "sessions.db-shm",
    "config.json",
    "known_games.json",
    "shortcuts.json",
    "config.json.bak",
];

/// Защищён ли файл. Сравнение без учёта регистра: Windows не различает
/// `Sessions.DB` и `sessions.db`, значит и защита должна быть одинаковой.
pub fn is_protected(name: &str) -> bool {
    let n = name.trim();
    PROTECTED_FILES
        .iter()
        .any(|p| p.eq_ignore_ascii_case(n))
}

/// Проверка имени файла из манифеста.
///
/// Имя приходит из сети, поэтому пускать его в файловую систему нельзя
/// без проверки. Пропускаем только простые имена файлов: без путей,
/// без `..`, без диска, без спецсимволов.
pub fn is_safe_rel_name(name: &str) -> bool {
    let n = name.trim();
    if n.is_empty() || n.len() > 128 {
        return false;
    }
    // Не путь, а именно имя: ни разделителей, ни диска, ни выхода вверх.
    if n.contains('/') || n.contains('\\') || n.contains(':') {
        return false;
    }
    if n == "." || n == ".." || n.starts_with("..") {
        return false;
    }
    // Недопустимые символы Windows.
    if n.chars().any(|c| {
        matches!(
            c,
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0' | '/' | '\\'
        )
    }) {
        return false;
    }
    // Должно быть расширение — иначе это нечто странное (напр. `README`).
    n.contains('.')
}

// ---------------------------------------------------------------------------
// Манифест
// ---------------------------------------------------------------------------

/// Описание обновления, пришедшее с GitHub.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Manifest {
    /// Версия сборки, для которой сделан манифест.
    pub version: String,
    /// Сборка (дата) в формате YYYYMMDD.
    #[serde(default)]
    pub build: String,
    /// Файлы, которые изменились: имя и новый хеш (SHA-256).
    /// Только их и скачивает обновление.
    #[serde(default)]
    pub files: Vec<FileEntry>,
    /// Краткий changelog версии для диалога подтверждения.
    /// Генерируется tools/make-manifest.ps1 из CHANGELOG.md.
    /// Старые манифесты поля не имеют — None (диалог показывает fallback).
    #[serde(default)]
    pub changelog: Option<String>,
}

/// Один изменившийся файл.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FileEntry {
    /// Имя файла относительно каталога программы.
    pub name: String,
    /// SHA-256 содержимого — по нему решаем, совпадает ли файл уже с тем,
    /// что стоит на диске (тогда не качаем) и не побился ли при загрузке.
    #[serde(default)]
    pub sha256: String,
}

impl Manifest {
    /// Разбор манифеста из строки JSON.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let m: Manifest =
            serde_json::from_str(text).map_err(|e| format!("манифест не разобран: {e}"))?;
        m.validate()?;
        Ok(m)
    }

    /// Проверка манифеста перед использованием: имена файлов должны быть
    /// безопасными, защищённые — отсутствовать.
    ///
    /// Смысл: даже если сам манифест подписан тем, кто залил релиз, нельзя
    /// позволить ему указать `sessions.db` — иначе обновление удалит
    /// историю сессий. Это осознанно жёсткое ограничение.
    pub fn validate(&self) -> Result<(), String> {
        if self.version.trim().is_empty() {
            return Err("в манифесте нет версии".to_string());
        }
        for f in &self.files {
            if !is_safe_rel_name(&f.name) {
                return Err(format!("небезопасное имя файла: {:?}", f.name));
            }
            if is_protected(&f.name) {
                return Err(format!(
                    "манифест пытается обновить пользовательский файл: {:?}",
                    f.name
                ));
            }
            if !f.name.to_ascii_lowercase().ends_with(".exe") {
                return Err(format!("обновлять можно только .exe: {:?}", f.name));
            }
        }
        Ok(())
    }

    /// Файлы, которые реально нужно качать: валидные имена, без дублей.
    /// Дубль отбрасываем — иначе один файл скачался бы дважды.
    pub fn wanted_files(&self) -> Vec<&FileEntry> {
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for f in &self.files {
            if !is_safe_rel_name(&f.name) || is_protected(&f.name) {
                continue;
            }
            let key = f.name.to_ascii_lowercase();
            if seen.iter().any(|s| s == &key) {
                continue;
            }
            seen.push(key);
            out.push(f);
        }
        out
    }

    /// Человеческое описание изменений для интерфейса.
    pub fn describe(&self) -> String {
        if self.files.is_empty() {
            return format!("Обновление до {}: файлы не изменились.", self.version);
        }
        let names: Vec<String> = self
            .wanted_files()
            .iter()
            .map(|f| f.name.clone())
            .collect();
        format!(
            "Доступно обновление до {} (сборка {}). Файлов: {}.",
            self.version,
            if self.build.is_empty() { "—".to_string() } else { self.build.clone() },
            names.join(", ")
        )
    }
}

// ---------------------------------------------------------------------------
// Расписание проверки
// ---------------------------------------------------------------------------

/// Как часто проверять обновления.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateFreq {
    /// Каждый час.
    Hourly,
    /// Раз в сутки.
    Daily,
    /// Раз в неделю.
    Weekly,
    /// Никогда, только вручную.
    Manual,
}

impl UpdateFreq {
    /// Интервал между проверками в секундах. `Manual` = `None`.
    pub fn interval_secs(self) -> Option<u64> {
        match self {
            UpdateFreq::Hourly => Some(3600),
            UpdateFreq::Daily => Some(24 * 3600),
            UpdateFreq::Weekly => Some(7 * 24 * 3600),
            UpdateFreq::Manual => None,
        }
    }

    /// Подпись для интерфейса.
    pub fn label(self) -> &'static str {
        match self {
            UpdateFreq::Hourly => "Каждый час",
            UpdateFreq::Daily => "Ежедневно",
            UpdateFreq::Weekly => "Раз в неделю",
            UpdateFreq::Manual => "Вручную (не проверять)",
        }
    }

    /// Разбор значения из config.json.
    pub fn from_code(code: u8) -> UpdateFreq {
        match code {
            1 => UpdateFreq::Daily,
            2 => UpdateFreq::Weekly,
            3 => UpdateFreq::Manual,
            _ => UpdateFreq::Hourly,
        }
    }

    /// Значение для config.json.
    pub fn code(self) -> u8 {
        match self {
            UpdateFreq::Hourly => 0,
            UpdateFreq::Daily => 1,
            UpdateFreq::Weekly => 2,
            UpdateFreq::Manual => 3,
        }
    }

    /// Все варианты для выпадающего списка.
    pub fn all() -> [UpdateFreq; 4] {
        [
            UpdateFreq::Hourly,
            UpdateFreq::Daily,
            UpdateFreq::Weekly,
            UpdateFreq::Manual,
        ]
    }
}

// ---------------------------------------------------------------------------
// Пути
// ---------------------------------------------------------------------------

/// Каталог, где лежит программа (каталог её исполняемого файла).
pub fn program_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        // Запасной вариант: каталог, откуда запустили.
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Папка для загрузки: рядом с программой, а не в системном TEMP.
///
/// Рядом с программой всегда можно писать (иначе программа вообще не
/// обновилась бы из Program Files), и файлы видны рядом с ней, а где-то
/// в недрах `AppData\Local\Temp`.
pub fn tmp_dir(base: &Path) -> PathBuf {
    base.join(TMP_DIR_NAME)
}

// ---------------------------------------------------------------------------
// Хеширование (SHA-256)
// ---------------------------------------------------------------------------

/// SHA-256 файла в нижнем регистре hex.
///
/// Своя реализация, потому что крейт `sha2` в зависимостях нет, а весит
/// он мало и тут единственное, что реально нужно: проверить, что
/// скачанный файл не побился и действительно ли он новой версии.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.hex())
}

/// SHA-256 данных.
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.hex()
}

/// Минимальный SHA-256 (FIPS 180-4). Только хеширование, без HMAC.
pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in self
            .state
            .iter_mut()
            .zip([a, b, c, d, e, f, g, h].into_iter())
        {
            *s = s.wrapping_add(v);
        }
    }

    pub fn hex(mut self) -> String {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        // update увеличил total — это не важно, длину считаем отдельно.
        while self.buf_len != 56 {
            self.update(&[0x00]);
        }
        let block_tail = {
            let mut b = self.buf;
            b[56..64].copy_from_slice(&bits.to_be_bytes());
            b
        };
        self.compress(&block_tail);
        let mut out = String::with_capacity(64);
        for v in self.state {
            out += &format!("{v:08x}");
        }
        out
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Что нужно скачать
// ---------------------------------------------------------------------------

/// План обновления: какие файлы качать, а какие уже совпадают.
///
/// `to_download` — файлы, которых на диске нет или хеш не совпал.
/// `unchanged` — совпали, качать не нужно.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdatePlan {
    pub to_download: Vec<FileEntry>,
    pub unchanged: Vec<String>,
    /// Защищённые файлы, найденные рядом с программой. Их не трогаем —
    /// это пользовательские данные, и полезно знать, что они на месте.
    pub preserved: Vec<String>,
}

/// Разница между манифестом и тем, что лежит на диске.
pub fn plan_update(base: &Path, manifest: &Manifest) -> UpdatePlan {
    let mut to_download = Vec::new();
    let mut unchanged = Vec::new();
    for f in manifest.wanted_files() {
        // Защищённое не качаем в любом случае.
        if is_protected(&f.name) {
            continue;
        }
        let local = base.join(&f.name);
        let same = local.is_file()
            && sha256_file(&local)
                .map(|h| h.eq_ignore_ascii_case(&f.sha256))
                .unwrap_or(false);
        if same {
            unchanged.push(f.name.clone());
        } else {
            to_download.push(f.clone());
        }
    }
    // Что из пользовательских файлов лежит рядом — показываем, что они
    // сохранятся при обновлении.
    let mut preserved = Vec::new();
    for p in PROTECTED_FILES {
        if base.join(p).exists() {
            preserved.push((*p).to_string());
        }
    }
    UpdatePlan {
        to_download,
        unchanged,
        preserved,
    }
}

// ---------------------------------------------------------------------------
// Сеть: проверка и загрузка
// ---------------------------------------------------------------------------

/// Адрес манифеста для указанного канала.
pub fn manifest_url_for(channel: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{REPO}/{}/{MANIFEST_NAME}",
        branch_for(channel)
    )
}

/// Адрес файла сборки в ветке канала (raw отдаёт без редиректов).
pub fn file_url_for(channel: &str, name: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{REPO}/{}/{name}",
        branch_for(channel)
    )
}

/// Адрес манифеста по умолчанию (стабильный канал).
pub fn manifest_url() -> String {
    manifest_url_for(CHANNEL_STABLE)
}

/// Адрес файла по умолчанию (стабильный канал).
pub fn file_url(name: &str) -> String {
    file_url_for(CHANNEL_STABLE, name)
}

fn client(timeout_secs: u64) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        // connect_timeout короче — обрыв на TCP не должен ждать весь read-таймаут.
        .connect_timeout(std::time::Duration::from_secs(15))
        .user_agent("TraySession-Updater/0.7")
        .build()
        .map_err(|e| format!("HTTP-клиент: {e}"))
}

/// Проверить соединение с GitHub (блокирующий вызов — запускать в фоне).
pub fn check_github_connection() -> Result<String, String> {
    let c = client(10)?;
    let resp = c
        .get("https://api.github.com/")
        .send()
        .map_err(|e| format!("Нет соединения с GitHub: {e}"))?;
    let status = resp.status();
    if status.is_success() {
        Ok(format!("Соединение с GitHub: OK ({status})"))
    } else {
        Err(format!("GitHub ответил: {status}"))
    }
}

/// Скачать манифест указанного канала и разобрать его.
pub fn fetch_manifest_for(channel: &str) -> Result<Manifest, String> {
    fetch_manifest_for_timeout(channel, 20)
}

/// То же, с заданным таймаутом. Pipeline берёт 30 с (R3 шаг 4).
pub fn fetch_manifest_for_timeout(channel: &str, timeout_secs: u64) -> Result<Manifest, String> {
    let c = client(timeout_secs)?;
    let url = manifest_url_for(channel);
    let text = c
        .get(&url)
        .send()
        .map_err(|e| format!("не скачался манифест {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("манифест недоступен {url}: {e}"))?
        .text()
        .map_err(|e| format!("манифест не прочитан {url}: {e}"))?;
    Manifest::parse(&text)
}

/// Скачать манифест стабильного канала (как раньше).
/// Ручная проверка переведена на fetch_manifest_for с каналом из настроек;
/// оставлена как короткий синоним стабильного канала.
#[allow(dead_code)]
pub fn fetch_manifest() -> Result<Manifest, String> {
    fetch_manifest_for(CHANNEL_STABLE)
}

/// Скачать один файл в `dest` и проверить его SHA-256.
///
/// С retry: живой тест показал обрыв соединения посреди 10-МБ тела
/// ("error decoding response body" — это hyper IncompleteMessage, а не
/// проблема декодирования). Возвращает размер записанного файла.
pub fn download_to(
    base: &Path,
    url: &str,
    dest: &Path,
    expect_sha: &str,
) -> Result<u64, String> {
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match try_download_once(url, dest, expect_sha, attempt) {
            Ok(n) => return Ok(n),
            Err(e) => {
                last_err = e;
                if attempt < MAX_ATTEMPTS {
                    let delay = std::time::Duration::from_secs(retry_delay_secs(attempt));
                    ulog(
                        base,
                        "WARN",
                        &format!(
                            "попытка {attempt}/{MAX_ATTEMPTS} скачать {url} не удалась: {last_err}. Повтор через {delay:?}"
                        ),
                    );
                    std::thread::sleep(delay);
                }
            }
        }
    }
    Err(format!(
        "не удалось скачать {url} за {MAX_ATTEMPTS} попытки. Последняя ошибка: {last_err}"
    ))
}

/// Задержка перед повтором скачивания, секунды: 1, 2, 4, ...
/// Вынесена именованной, чтобы расписание проверялось тестом, а не
/// числом в уме при чтении цикла.
fn retry_delay_secs(attempt: u32) -> u64 {
    1 << attempt.saturating_sub(1).min(6)
}

fn try_download_once(
    url: &str,
    dest: &Path,
    expect_sha: &str,
    attempt: u32,
) -> Result<u64, String> {
    // Каждый раз новый клиент — старый мог кэшировать broken connection.
    // Таймаут 180: 10 МБ с raw.githubusercontent.com может идти медленно.
    let c = client(180)?;
    let resp = c
        .get(url)
        .send()
        .map_err(|e| format!("attempt {attempt}: отправка запроса: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("attempt {attempt}: HTTP {status} для {url}"));
    }
    // Content-Length для диагностики (может отсутствовать при chunked).
    let expected_len = resp.content_length();
    let bytes = resp.bytes().map_err(|e| {
        let got = expected_len
            .map(|n| format!("ожидалось {n} байт, "))
            .unwrap_or_default();
        format!("attempt {attempt}: тело оборвано ({got}error: {e})")
    })?;
    if let Some(n) = expected_len {
        if bytes.len() as u64 != n {
            return Err(format!(
                "attempt {attempt}: получено {} байт из {n} — соединение оборвано",
                bytes.len()
            ));
        }
    }
    // Хеш проверяем ДО записи: битый файл на диск не попадает.
    if !expect_sha.trim().is_empty() {
        let got = sha256_bytes(&bytes);
        if !got.eq_ignore_ascii_case(expect_sha.trim()) {
            return Err(format!(
                "attempt {attempt}: хеш не совпал (ожидали {}, получили {}) — файл побился при загрузке",
                expect_sha.trim(),
                got
            ));
        }
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("attempt {attempt}: не создана папка {}: {e}", dir.display()))?;
    }
    std::fs::write(dest, &bytes)
        .map_err(|e| format!("attempt {attempt}: не записан {}: {e}", dest.display()))?;
    Ok(bytes.len() as u64)
}

// ---------------------------------------------------------------------------
// Отчёт фонового процесса
// ---------------------------------------------------------------------------

/// Итог обновления, который основной процесс читает из файла.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct UpdateReport {
    pub ok: bool,
    /// Человеческий текст: что обновилось или почему нет.
    pub message: String,
    /// Имена файлов, которые заменены.
    #[serde(default)]
    pub updated: Vec<String>,
    /// Имена файлов, которые совпали и не качались.
    #[serde(default)]
    pub unchanged: Vec<String>,
    /// Пользовательские файлы, найденные рядом и сохранённые.
    #[serde(default)]
    pub preserved: Vec<String>,
    /// Этап pipeline, на котором остановился updater
    /// (mutex, reexec, wait_parent, manifest, plan, download,
    /// replace, spawn, cleanup, done). Пусто у старых отчётов.
    #[serde(default)]
    pub stage: String,
    /// Машинный вид ошибки (timeout, http, hash, io, spawn...).
    /// Пусто при успехе и у старых отчётов.
    #[serde(default)]
    pub error_kind: String,
}

impl UpdateReport {
    fn save(&self, base: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // Папка может быть ещё не создана (первый запуск, отчёт пишет
        // основной процесс, а он не создавал update_tmp).
        let dir = tmp_dir(base);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(REPORT_NAME), text)
    }
    pub fn load(base: &Path) -> Option<UpdateReport> {
        let text = std::fs::read_to_string(tmp_dir(base).join(REPORT_NAME)).ok()?;
        serde_json::from_str(&text).ok()
    }
    pub fn clear(base: &Path) {
        let _ = std::fs::remove_file(tmp_dir(base).join(REPORT_NAME));
    }
    /// Текст для показа в интерфейсе.
    ///
    /// Про сохранность данных говорится ВСЕГДА, когда обновление удалось:
    /// это то, чего пользователь ждёт больше всего, и молчать об этом после
    /// замены файлов нельзя.
    pub fn describe(&self) -> String {
        if !self.ok {
            return format!("Обновление не выполнено: {}.", self.message);
        }
        let mut s = if self.updated.is_empty() {
            format!("Обновление: {}.", self.message)
        } else {
            format!("Обновлено: {}.", self.updated.join(", "))
        };
        if !self.unchanged.is_empty() {
            s += &format!(" Без изменений: {}.", self.unchanged.join(", "));
        }
        if !self.preserved.is_empty() {
            s += &format!(" Ваши данные сохранены: {}.", self.preserved.join(", "));
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Планирование замены файлов
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Атомарная замена файлов (.old/.new с откатом)
// ---------------------------------------------------------------------------

/// Заменить файлы программы на скачанные — атомарно, с откатом.
///
/// Протокол для каждого файла (скачанное лежит как `<name>.new` в `tmp`):
///   a. Удалить `<name>.old` (остаток прошлой попытки).
///   b. Rename `<name>` -> `<name>.old` (освобождает имя; rename живого
///      exe Windows разрешает, а перезапись — нет).
///   c. Rename `<name>.new` -> `<name>`.
/// При ошибке b/c — откат: всё уже переименованное возвращается
/// `<name>.old` -> `<name>`, `.new` удаляется, возвращается ошибка.
/// Защищённые и небезопасные имена пропускаются молча (план их уже
/// отфильтровал — это вторая линия обороны).
///
/// Возвращает имена заменённых файлов.
pub fn replace_files_atomic(
    base: &Path,
    tmp: &Path,
    files: &[FileEntry],
) -> Result<Vec<String>, String> {
    let mut done: Vec<String> = Vec::new();
    for f in files {
        if is_protected(&f.name) || !is_safe_rel_name(&f.name) {
            continue;
        }
        let staged = tmp.join(format!("{}.new", f.name));
        if !staged.is_file() {
            rollback_replaced(base, &done);
            return Err(format!("нет скачанного файла: {}", staged.display()));
        }
        let dst = base.join(&f.name);
        let old = base.join(format!("{}.old", f.name));
        // a. Чистим остаток прошлой попытки. Ошибка здесь тоже
        // откатывает уже заменённое: частичный прогресс недопустим.
        if old.exists() {
            if let Err(e) = std::fs::remove_file(&old) {
                rollback_replaced(base, &done);
                return Err(format!("не удалён остаток {}: {e}", old.display()));
            }
        }
        // b. Уводим текущий файл в сторону. Его может не быть (первая
        // установка) — тогда откатывать нечего, идём к c.
        let had_current = dst.exists();
        if had_current {
            if let Err(e) = std::fs::rename(&dst, &old) {
                rollback_replaced(base, &done);
                return Err(format!(
                    "не убран с пути {} -> {}: {e}",
                    dst.display(),
                    old.display()
                ));
            }
        }
        // c. Ставим новый на место.
        if let Err(e) = std::fs::rename(&staged, &dst) {
            if had_current {
                let _ = std::fs::rename(&old, &dst);
            }
            rollback_replaced(base, &done);
            return Err(format!(
                "не поставлен на место {} -> {}: {e}",
                staged.display(),
                dst.display()
            ));
        }
        done.push(f.name.clone());
    }
    Ok(done)
}

/// Откат уже заменённых файлов: `<name>.old` -> `<name>`.
fn rollback_replaced(base: &Path, done: &[String]) {
    for name in done.iter().rev() {
        let dst = base.join(name);
        let old = base.join(format!("{name}.old"));
        if old.exists() {
            let _ = std::fs::rename(&old, &dst);
        }
    }
}

/// Убрать `.old`-копии после успешной замены (шаг cleanup pipeline).
pub fn cleanup_old_files(base: &Path, names: &[String]) {
    for name in names {
        let old = base.join(format!("{name}.old"));
        let _ = std::fs::remove_file(&old);
    }
}

// ---------------------------------------------------------------------------
// Фоновый процесс
// ---------------------------------------------------------------------------

/// Прочитать аргументы командной строки: запущен ли updater и с какими
/// параметрами.
///
/// Формат pipeline: `--updater --parent-pid <PID> --wait-secs <N>
/// --channel <stable|beta>`. Всё именованное, у каждого значения есть
/// разумное умолчание (PID 0 = ждать некого, 60 с, stable) — updater
/// никогда не падает на разборе аргументов, а пишет отчёт и выходит
/// по коду pipeline.
pub fn parse_args(args: &[String]) -> Option<UpdateArgs> {
    if !args.iter().any(|a| a == UPDATER_FLAG) {
        return None;
    }
    let value_of = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    Some(UpdateArgs {
        parent_pid: value_of("--parent-pid")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0),
        wait_secs: value_of("--wait-secs")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60),
        // Канал передавать обязательно: фоновый процесс — отдельный, и без
        // явного канала он обновил бы программу по стабильной ветке.
        channel: value_of("--channel").unwrap_or_else(|| CHANNEL_STABLE.to_string()),
        // Корень программы: reexec-копия работает из update_tmp, и её
        // program_dir() указывает туда же — весь pipeline (отчёт, загрузки,
        // замена, spawn, логи) ушёл бы мимо корня. Поэтому base едет явным
        // аргументом от main, который знает свой каталог точно.
        base_dir: value_of("--base").map(PathBuf::from).unwrap_or_else(program_dir),
    })
}

/// Флаг, которым основная программа запускает сама себя как updater.
pub const UPDATER_FLAG: &str = "--updater";
/// Флаг перезапуска после обновления: main с ним обходит C5-mutex
/// через завершение старого процесса (см. R4), а не через показ окна.
pub const UPDATED_FLAG: &str = "--updated";

/// Параметры фонового процесса.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateArgs {
    /// PID main-процесса: updater ждёт его смерти перед заменой.
    /// 0 = ждать некого (ручной запуск updater для диагностики).
    pub parent_pid: u32,
    /// Сколько секунд ждать смерти main-процесса.
    pub wait_secs: u64,
    /// Канал обновлений: `stable` или `beta`.
    pub channel: String,
    /// Корень программы (каталог заменяемых файлов). Едет явным аргументом,
    /// т.к. после шага 2 (reexec из update_tmp) program_dir() копии
    /// указывает в update_tmp, а не в корень.
    pub base_dir: PathBuf,
}

/// Имя, под которым фоновый процесс работает после шага 2 pipeline:
/// копия себя в update_tmp (имя TraySession.exe тем самым свободно
/// для замены — запущенный файл Windows перезаписать не даёт).
pub const UPDATER_NAME: &str = "_updater_running.exe";

/// Убрать себя с пути, освободив имя `TraySession.exe` под новую сборку.
///
/// Windows не позволяет ЗАМЕНИТЬ файл, который сейчас выполняется, но
/// позволяет его ПЕРЕИМЕНОВАТЬ. Без этого фоновый процесс не смог бы
/// обновить самого себя: он и есть тот файл, который требуется перезаписать
/// (ошибка «процесс не может получить доступ к файлу», os error 32).
///
/// Удалить переименованную копию фонового процесса после его выхода.
///
/// Windows не даёт удалить исполняемый файл, пока он запущен, поэтому
/// удаление откладывается и делается сторонним процессом.
pub fn remove_aside_delayed(path: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW, чтобы не мигало окно консоли.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/C", "ping", "127.0.0.1", "-n", "3", ">", "nul", "&", "del", "/F", "/Q"])
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

// ---------------------------------------------------------------------------
// Updater pipeline (единый последовательный процесс, R3)
// ---------------------------------------------------------------------------

/// Имя отдельного лога updater (R6). Шаги pipeline не должны теряться
/// в main-логе. log.rs не трогаем — пишем прямым append из update.rs.
pub const UPDATER_LOG_NAME: &str = "updater.log";

/// Одна строка в updater.log. Никогда не паникует и не мешает pipeline:
/// ошибки записи молча пропускаются (лог не должен ронять обновление).
pub fn ulog(base: &Path, level: &str, msg: &str) {
    let line = format!(
        "{} [{}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        level,
        msg.replace(['\r', '\n'], " ")
    );
    let dir = base.join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(UPDATER_LOG_NAME))
    {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

/// Завершить pipeline с ошибкой: отчёт + лог + код возврата.
/// Единая точка выхода — каждый шаг пользуется ею, поэтому ни один путь
/// не забывает ни отчёт, ни лог.
fn fail(base: &Path, stage: &str, kind: &str, message: String, code: i32) -> i32 {
    ulog(base, "ERROR", &format!("[{stage}] {message}"));
    let _ = write_report(
        base,
        UpdateReport {
            ok: false,
            message,
            stage: stage.to_string(),
            error_kind: kind.to_string(),
            ..Default::default()
        },
    );
    code
}

/// Имя файла с PID владельца updater-mutex (диагностика + stale-detect,
/// FIX 2). Пишется сразу после захвата mutex, удаляется на шаге cleanup
/// при успехе. Если updater прибили (Stop-Process) между захватом и
/// cleanup — файл остаётся и следующий запуск по нему понимает, жив ли
/// владелец.
pub const UPDATER_OWNER_FILE: &str = "updater_owner.txt";

/// RAII-держатель mutex updater (FIX 1): закрывает handle при любом выходе
/// из run_updater_inner. Раньше handle не закрывался вовсе.
#[cfg(windows)]
struct UpdaterMutexGuard(winapi::um::winnt::HANDLE);

#[cfg(windows)]
impl Drop for UpdaterMutexGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                winapi::um::handleapi::CloseHandle(self.0);
            }
        }
    }
}

#[cfg(not(windows))]
struct UpdaterMutexGuard;

/// Состояние владельца updater-mutex (FIX 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerState {
    /// Владелец жив — mutex занят по-настоящему, выходим с диагностикой.
    Alive(u32),
    /// Владелец мёртв — owner-файл остался от прибитого прогона,
    /// ретраим захват.
    Dead(u32),
    /// Owner-файла нет или PID не прочитать — сказать нечего.
    Unknown,
}

/// Жив ли владелец mutex по owner-файлу.
///
/// Мёртвый процесс держать mutex НЕ может (ОС закрывает все хендлы при
/// завершении, даже при Stop-Process) — поэтому Dead здесь означает:
/// файл остался от прибитого прогона, а сам mutex уже свободен или
/// вот-вот освободится. Проверяем именно процесс, а не mutex.
fn updater_mutex_owner_state(base: &Path) -> OwnerState {
    let pid: u32 = match std::fs::read_to_string(tmp_dir(base).join(UPDATER_OWNER_FILE))
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        Some(p) if p != 0 => p,
        _ => return OwnerState::Unknown,
    };
    if wait_for_pid_death(pid, std::time::Duration::from_millis(0)) {
        OwnerState::Dead(pid)
    } else {
        OwnerState::Alive(pid)
    }
}

/// Шаг 1: захватить mutex updater.
///
/// Ok(guard) — держим до конца pipeline; guard живёт до выхода из
/// run_updater_inner, Drop закрывает handle (FIX 1).
/// Err(message) — mutex занят: внутри уже выполнена stale-проверка
/// (FIX 2), вызывающий пишет fail() с диагностикой (FIX 3).
/// Handle чужого mutex, полученный при ERROR_ALREADY_EXISTS, закрываем
/// сразу — иначе мы сами держим его открытым.
#[cfg(windows)]
fn acquire_updater_mutex(base: &Path) -> Result<UpdaterMutexGuard, String> {
    use winapi::shared::winerror::ERROR_ALREADY_EXISTS;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::synchapi::CreateMutexW;
    let name: Vec<u16> = "Local\\TraySession_Updater\0".encode_utf16().collect();
    for attempt in 1..=3u32 {
        let h = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
        if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
            if h.is_null() {
                return Err("не создан mutex updater".to_string());
            }
            // Мы владельцы: фиксируем PID для диагностики и stale-detect.
            // Папку создаём здесь же: шаг 1 идёт до create_dir_all(tmp),
            // а на чистой установке update_tmp ещё нет.
            let owner = tmp_dir(base).join(UPDATER_OWNER_FILE);
            if let Some(parent) = owner.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&owner, std::process::id().to_string());
            return Ok(UpdaterMutexGuard(h));
        }
        // Чужой handle закрываем сразу — держать его значит
        // удерживать чужой mutex открытым.
        if !h.is_null() {
            unsafe {
                CloseHandle(h);
            }
        }
        match updater_mutex_owner_state(base) {
            OwnerState::Dead(pid) => {
                ulog(
                    base,
                    "WARN",
                    &format!("mutex занят, но владелец pid={pid} мёртв (попытка {attempt}/3)"),
                );
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            OwnerState::Alive(pid) => {
                return Err(format!(
                    "updater mutex held. owner PID: {pid}. Если процесс жив — дождитесь конца обновления; если мёртв — mutex освободится сам, повторите."
                ));
            }
            OwnerState::Unknown => {
                return Err(
                    "updater mutex held. owner PID: unknown. Если обновление точно не идёт — mutex освободится после перезагрузки."
                        .to_string(),
                );
            }
        }
    }
    Err(
        "updater mutex held by dead process, try reboot: владелец мёртв, но mutex всё ещё занят."
            .to_string(),
    )
}

#[cfg(not(windows))]
fn acquire_updater_mutex(_base: &Path) -> Result<UpdaterMutexGuard, String> {
    Ok(UpdaterMutexGuard)
}

/// Точка входа фонового процесса: единый pipeline из 10 шагов (R3).
/// На любом шаге возможен только один исход: следующий шаг или fail().
/// Паники не ожидаются, но если что-то запаниковало — main уже вышел,
/// файлы целы (замена атомарна через .old), следующий запуск начнёт
/// с чистого pipeline.
pub fn run_updater(base: &Path, parent_pid: u32, wait_secs: u64, channel: &str) -> i32 {
    run_updater_inner(base, parent_pid, wait_secs, channel)
}

/// Шаг 2: уйти с имени TraySession.exe.
///
/// Windows не даёт ПЕРЕЗАПИСАТЬ запущенный файл (os error 32), поэтому
/// updater копирует себя в update_tmp/_updater_running.exe и
/// перезапускается оттуда с теми же аргументами; родитель завершается.
/// Уже запущены под этим именем — пропускаем.
fn reexec_as_updater(base: &Path) -> bool {
    let me = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if me.file_name().map(|n| n == UPDATER_NAME).unwrap_or(false) {
        return false;
    }
    let aside = tmp_dir(base).join(UPDATER_NAME);
    if let Some(d) = aside.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if std::fs::copy(&me, &aside).is_err() {
        return false;
    }
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let _ = std::process::Command::new(&aside).args(&argv).spawn();
    true
}

/// Основная работа обновления: шаги 1–10 pipeline (R3).
/// Возвращает код выхода (0 = успех, иначе см. fail() на каждом шаге).
fn run_updater_inner(base: &Path, parent_pid: u32, wait_secs: u64, channel: &str) -> i32 {
    let base = base.to_path_buf();
    // Шаг 1: mutex. Имя отличается от C5-мутекса main-окна: main + updater
    // обязаны сосуществовать, C5 намеренно пропускает --updater.
    // (Наследие C14: два параллельных процесса делили отчёт и файлы.)
    // Guard закрывает handle при любом выходе (FIX 1); при занятости
    // сначала stale-проверка по owner-PID (FIX 2), потом fail с
    // диагностикой (FIX 3).
    #[cfg(windows)]
    let _updater_mutex = match acquire_updater_mutex(&base) {
        Ok(g) => g,
        Err(message) => {
            return fail(&base, "mutex", "busy", message, 0);
        }
    };
    ulog(
        &base,
        "INFO",
        &format!(
            "updater запущен: канал {}, parent_pid={parent_pid}, ждём до {wait_secs} с",
            channel_label(channel)
        ),
    );
    // Шаг 2: уйти с имени TraySession.exe (копия + перезапуск оттуда).
    if reexec_as_updater(&base) {
        ulog(&base, "INFO", "перезапущен из update_tmp, родитель выходит");
        return 0;
    }
    let tmp = tmp_dir(&base);
    if let Err(e) = std::fs::create_dir_all(&tmp) {
        return fail(
            &base,
            "tmp",
            "io",
            format!("не создана папка загрузки {}: {e}", tmp.display()),
            2,
        );
    }
    // Отчёт всегда свежий: старый мог остаться от прошлого обновления.
    // (Сценарий C ручной проверки опирается на это: прибитый updater
    // не должен оставлять ok=true от прошлого раза.)
    UpdateReport::clear(&base);
    // Шаг 3: дождаться смерти main. Main выходит сам сразу после spawn
    // (R2) — долгого ожидания в норме нет. Fallback — main_exited.flag:
    // если PID успели переиспользовать, а флаг есть — main подтвердил
    // готовность. Флаг съедаем сразу, чтобы протухший не разрешил
    // замену при живом процессе (та же ловушка, что была у READY_FLAG).
    let exited_flag = tmp_dir(&base).join(MAIN_EXITED_FLAG);
    let exited_flag_present = exited_flag.exists();
    let _ = std::fs::remove_file(&exited_flag);
    if parent_pid != 0 {
        ulog(&base, "INFO", &format!("жду смерти main pid={parent_pid} до {wait_secs} с"));
        if !wait_for_pid_death(parent_pid, std::time::Duration::from_secs(wait_secs)) {
            if exited_flag_present {
                ulog(
                    &base,
                    "WARN",
                    "main жив по PID, но main_exited.flag был — считаю готовым",
                );
            } else {
                return fail(
                    &base,
                    "wait_parent",
                    "timeout",
                    format!("main pid={parent_pid} не вышел за {wait_secs} с, файлы не тронуты"),
                    3,
                );
            }
        } else {
            ulog(&base, "INFO", "main мёртв, идём дальше");
        }
    } else {
        ulog(&base, "WARN", "parent_pid=0 (диагностика?), ожидание пропущено");
    }
    // Шаг 4. Манифест канала (таймаут 30 с).
    let url = manifest_url_for(channel);
    ulog(&base, "INFO", &format!("запрашиваю манифест: {url}"));
    let manifest = match fetch_manifest_for_timeout(channel, 30) {
        Ok(m) => {
            ulog(
                &base,
                "INFO",
                &format!(
                    "манифест получен: версия {}, сборка {}, файлов: {}",
                    m.version,
                    m.build,
                    m.files.len()
                ),
            );
            m
        }
        Err(e) => {
            return fail(&base, "manifest", "http", format!("манифест не получен: {e}"), 4);
        }
    };
    // Шаг 5. План: какие файлы отличаются от диска.
    let plan = plan_update(&base, &manifest);
    ulog(
        &base,
        "INFO",
        &format!(
            "план: скачать {}, совпадают {}, данных пользователя рядом: {}",
            plan.to_download.len(),
            plan.unchanged.len(),
            plan.preserved.len()
        ),
    );
    for f in &plan.to_download {
        ulog(&base, "INFO", &format!("  к замене: {}", f.name));
    }
    for f in &plan.unchanged {
        ulog(&base, "INFO", &format!("  без изменений: {f}"));
    }
    for f in &plan.preserved {
        ulog(&base, "INFO", &format!("  НЕ ТРОГАЕМ (данные пользователя): {f}"));
    }
    if plan.to_download.is_empty() {
        ulog(&base, "INFO", "обновлять нечего: файлы совпадают с манифестом");
        let _ = write_report(
            &base,
            UpdateReport {
                ok: true,
                unchanged: plan.unchanged.clone(),
                preserved: plan.preserved.clone(),
                message: format!("уже установлена версия {}", manifest.version),
                stage: "done".to_string(),
                ..Default::default()
            },
        );
        return 0;
    }
    // Шаг 6. Скачать во временную папку как <name>.new (не трогая боевые
    // файлы до атомарной замены на шаге 7).
    for f in &plan.to_download {
        let url = file_url_for(channel, &f.name);
        ulog(&base, "INFO", &format!("качаю {url}"));
        let staged = tmp.join(format!("{}.new", f.name));
        if let Err(e) = download_to(&base, &url, &staged, &f.sha256) {
            return fail(&base, "download", "net", format!("загрузка не удалась: {e}"), 6);
        }
        ulog(&base, "INFO", &format!("скачано и сверено по SHA-256: {}", f.name));
    }
    // Шаг 7. Атомарная замена через .old/.new с откатом.
    ulog(&base, "INFO", "заменяю файлы");
    let replaced = match replace_files_atomic(&base, &tmp, &plan.to_download) {
        Ok(names) => {
            ulog(&base, "INFO", &format!("заменено файлов: {}", names.join(", ")));
            names
        }
        Err(e) => {
            return fail(&base, "replace", "io", format!("замена не удалась: {e}"), 7);
        }
    };
    // Шаг 8. Spawn нового процесса с --updated (R4: обход C5-mutex).
    // Отчёт пишется ПОСЛЕ spawn (шаг 10): main уже вышел и отчёт не читает.
    let exe = base.join(MAIN_EXE);
    if let Err(e) = std::process::Command::new(&exe).arg(UPDATED_FLAG).spawn() {
        // Новый даже не стартовал — возвращаем старый exe из .old.
        let old = base.join(format!("{MAIN_EXE}.old"));
        if old.exists() {
            let _ = std::fs::rename(&old, &exe);
        }
        return fail(
            &base,
            "spawn",
            "spawn",
            format!(
                "не удалось запустить {}: {e}. Старый exe восстановлен из .old, запустите вручную",
                exe.display()
            ),
            8,
        );
    }
    ulog(&base, "INFO", &format!("новый процесс запущен: {}", exe.display()));
    // Шаг 9. Cleanup: .old-копии, своя копия updater (отложенно — файл
    // держит сам процесс), main_pid.txt (протухший PID страшнее
    // отсутствующего: ОС переиспользует PID).
    cleanup_old_files(&base, &replaced);
    let aside = tmp.join(UPDATER_NAME);
    if aside.is_file() {
        let _ = remove_aside_delayed(&aside);
    }
    let _ = std::fs::remove_file(tmp.join(MAIN_PID_FILE));
    // Owner-файл больше не нужен: mutex свободен (guard закроет handle
    // при выходе), следующий запуск запишет свой PID сам.
    let _ = std::fs::remove_file(tmp.join(UPDATER_OWNER_FILE));
    // Шаг 10. Отчёт об успехе.
    ulog(&base, "INFO", "обновление завершено");
    let _ = write_report(
        &base,
        UpdateReport {
            ok: true,
            updated: replaced,
            unchanged: plan.unchanged.clone(),
            preserved: plan.preserved.clone(),
            message: format!("установлена версия {}", manifest.version),
            stage: "done".to_string(),
            ..Default::default()
        },
    );
    0
}

fn write_report(base: &Path, r: UpdateReport) -> std::io::Result<()> {
    r.save(base)
}

/// Дождаться смерти процесса по PID через WinAPI.
///
/// Чистая функция ожидания (без чтения файлов — PID передаёт вызывающий),
/// поэтому тестируется напрямую: живой PID + короткий таймаут = false,
/// мёртвый PID = true.
#[cfg(windows)]
fn wait_for_pid_death(pid: u32, timeout: std::time::Duration) -> bool {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::synchapi::WaitForSingleObject;
    use winapi::um::winbase::WAIT_OBJECT_0;
    use winapi::um::winnt::SYNCHRONIZE;

    unsafe {
        let h = OpenProcess(SYNCHRONIZE, 0, pid);
        if h.is_null() {
            // Такого процесса нет — значит, уже мёртв (или PID
            // переиспользован и закрыт между проверками — всё равно идём).
            return true;
        }
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        let r = WaitForSingleObject(h, ms);
        CloseHandle(h);
        r == WAIT_OBJECT_0
    }
}

#[cfg(not(windows))]
fn wait_for_pid_death(_pid: u32, _timeout: std::time::Duration) -> bool {
    true
}

/// Запустить фоновый процесс обновления (новый протокол, R2).
///
/// `exe` — путь к своему исполняемому файлу, `parent_pid` — PID main
/// (updater ждёт его смерти), `wait_secs` — лимит ожидания (default 60).
/// Отдельный процесс нужен затем, что (а) интерфейс не должен висеть
/// на загрузке, (б) заменять `.exe` может только тот, кто его не держит
/// открытым. Main после spawn сразу выходит и ничего не ждёт.
pub fn spawn_updater(
    exe: &Path,
    parent_pid: u32,
    wait_secs: u64,
    channel: &str,
) -> std::io::Result<()> {
    std::process::Command::new(exe)
        .arg(UPDATER_FLAG)
        .arg("--parent-pid")
        .arg(parent_pid.to_string())
        .arg("--wait-secs")
        .arg(wait_secs.to_string())
        // Корень программы: reexec-копия не должна выводить его из своего
        // program_dir() (там будет update_tmp). Каталог берём из пути exe.
        .arg("--base")
        .arg(
            exe.parent()
                .unwrap_or(Path::new("."))
                .to_string_lossy()
                .into_owned(),
        )
        // Канал обязателен: без него фоновый процесс взял бы стабильную
        // ветку, и бета-сборка молча откатывалась бы на старую версию.
        .arg("--channel")
        .arg(if channel.eq_ignore_ascii_case(CHANNEL_BETA) {
            CHANNEL_BETA
        } else {
            CHANNEL_STABLE
        })
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests;

/// Разобран ли настоящий манифест проекта и проходит ли проверку.
///
/// Манифест генерируется `tools\make-manifest.ps1` и лежит в корне. Если
/// он сломан (забыли перегенерировать, испортили правкой), программа не
/// найдёт обновление вообще — поэтому это проверяется тестом, а не
/// «вроде работало».
#[cfg(test)]
const REAL_MANIFEST: &str = include_str!("../update_manifest.json");

#[cfg(test)]
mod real_manifest {
    use super::*;

    /// Сквозная проверка главного требования: обновление НЕ МОЖЕТ
    /// удалить историю сессий, даже если манифест на это попросит.
    ///
    /// Собирается полный сценарий: манифест требует обновить
    /// пользовательский файл → manifest.validate() его отвергает →
    /// wanted_files() его не возвращает → plan_update() его не качает →
    /// replace_files_atomic() его не трогает. Файл на диске после всего этого
    /// обязан остаться байт в байт.
    #[test]
    fn session_history_survives_an_attack_by_manifest() {
        // Каталог внутри проекта (target/test-tmp), а не в системном %TEMP%
        // на C:: там тесты создают файлы-«пользователи», а диск может быть
        // почти заполнен. `TempDir` удаляет каталог сам, даже если тест упал.
        let dir = super::tests::TempDir::new("history");
        let d = &dir.0;
        // «История» пользователя.
        let history = b"SESSION-HISTORY-THAT-MUST-SURVIVE";
        std::fs::write(d.join("sessions.db"), history).unwrap();
        std::fs::write(d.join("config.json"), b"{\"api_key\":\"SECRET\"}").unwrap();
        // «Старая» программа и «новая» в папке загрузки (как .new —
        // pipeline качает именно туда, см. шаг 6).
        std::fs::write(d.join("TraySession.exe"), b"OLD").unwrap();
        let tmp = d.join("update_tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("TraySession.exe.new"), b"NEW-BINARY-0123456789").unwrap();
        // В tmp подложим «новую» историю — обновление и её не должно
        // заметить: файл защищённый.
        std::fs::write(tmp.join("sessions.db"), b"CLOBBERED-ATTEMPT").unwrap();

        // Манифест злого умысла: просит и программу, и историю.
        let evil = Manifest {
            version: "9.9.9".into(),
            build: "20990101".into(),
            files: vec![
                FileEntry {
                    name: "TraySession.exe".into(),
                    sha256: sha256_bytes(b"NEW-BINARY-0123456789"),
                },
                FileEntry {
                    name: "sessions.db".into(),
                    sha256: sha256_bytes(b"CLOBBERED-ATTEMPT"),
                },
                FileEntry {
                    name: "config.json".into(),
                    sha256: sha256_bytes(b"{}"),
                },
            ],
            changelog: None,
        };
        // 1. Валидация отвергает весь манифест — а не «пропускает плохие».
        let err = evil.validate().expect_err("манифест с sessions.db принят!");
        assert!(err.contains("sessions.db"), "{err}");
        // 2. В план попадает только программа.
        let plan = plan_update(d, &evil);
        let names: Vec<&str> = plan.to_download.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["TraySession.exe"], "в плано пошёл лишний файл: {names:?}");
        // 3. Замена трогает только плановые файлы, даже если в tmp лежит
        //    «новая» история.
        let replaced = replace_files_atomic(d, &tmp, &plan.to_download).unwrap();
        assert_eq!(replaced, vec!["TraySession.exe".to_string()]);
        cleanup_old_files(d, &replaced);
        // 4. Проверка результата: программа обновилась, данные — нет.
        assert_eq!(
            std::fs::read(d.join("TraySession.exe")).unwrap(),
            b"NEW-BINARY-0123456789",
            "программа не обновилась"
        );
        assert_eq!(
            std::fs::read(d.join("sessions.db")).unwrap(),
            history,
            "ИСТОРИЯ СЕССИЙ ПОСТРАДАЛА"
        );
        assert_eq!(
            std::fs::read(d.join("config.json")).unwrap(),
            b"{\"api_key\":\"SECRET\"}",
            "НАСТРОЙКИ ПОСТРАДАЛИ"
        );
        // 5. Про обновлённый файл сказано, что заменён; про данные — что
        //    сохранены.
        let r = UpdateReport {
            ok: true,
            message: String::new(),
            updated: vec!["TraySession.exe".into()],
            unchanged: vec![],
            preserved: plan.preserved.clone(),
            stage: "done".into(),
            error_kind: String::new(),
        };
        let d = r.describe();
        assert!(d.contains("TraySession.exe"), "{d}");
        assert!(d.contains("sessions.db"), "в отчёте не сказано про сохранность: {d}");
    }

    #[test]
    fn project_manifest_is_valid() {
        let m = Manifest::parse(REAL_MANIFEST).expect("манифест проекта не разобран");
        m.validate().expect("манифест проекта не прошёл проверку");
        assert!(!m.version.trim().is_empty(), "в манифесте нет версии");
        // В манифесте должны быть обе сборки, иначе обновление заменит
        // только половину.
        let names: Vec<&str> = m.wanted_files().iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"TraySession.exe"),
            "в манифесте нет главной программы: {names:?}"
        );
        assert!(
            names.contains(&"Tray_session_setup.exe"),
            "в манифесте нет установщика: {names:?}"
        );
        // Ничего пользовательского в манифесте быть не должно.
        for f in &m.files {
            assert!(!is_protected(&f.name), "в манифесте есть {f:?}");
        }
    }

    #[test]
    fn project_manifest_version_matches_cargo() {
        // Расхождение версий означает, что манифест забыли перегенерировать
        // после бампа версии: программа предложила бы «обновление», которого
        // на самом деле нет.
        let m = Manifest::parse(REAL_MANIFEST).unwrap();
        let cargo = include_str!("../Cargo.toml");
        let expected = cargo
            .lines()
            .find(|l| l.trim_start().starts_with("version"))
            .and_then(|l| l.split('"').nth(1))
            .unwrap_or("");
        assert_eq!(
            m.version, expected,
            "версия в манифесте ({}) не совпадает с Cargo.toml ({expected}). Перегенерируй: tools\\make-manifest.ps1",
            m.version
        );
    }

    #[test]
    fn shipped_binary_really_contains_the_declared_version() {
        // Ловушка, в которую попал релиз 0.7.30. Манифест объявлял 0.7.30,
        // а в самом .exe был собран 0.7.29: релизную сборку сделали ДО
        // поднятия версии, а манифест сгенерировали ПОСЛЕ. Оба файла —
        // текстовые и согласованы между собой, поэтому прежние проверки
        // были довольны, а пользователь получал программу, которая
        // называет себя прошлой версией.
        //
        // Проверяем сам бинарник: в нём версия и номер сборки лежат
        // открытым текстом. Если сборки ещё нет — тест пропускает молча,
        // чтобы `cargo test` до `cargo build --release` не падал.
        let exe = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("release")
            .join("game-session-tracker.exe");
        if !exe.is_file() {
            eprintln!("релизной сборки нет — проверка пропущена: {}", exe.display());
            return;
        }
        let bytes = std::fs::read(&exe).expect("не прочитан exe");
        let text = String::from_utf8_lossy(&bytes);
        let cargo = include_str!("../Cargo.toml");
        let version = cargo
            .lines()
            .find(|l| l.trim_start().starts_with("version"))
            .and_then(|l| l.split('"').nth(1))
            .unwrap_or("");
        assert!(
            !version.is_empty(),
            "не нашёл версию в Cargo.toml"
        );
        assert!(
            text.contains(version),
            "в собранном .exe нет версии {version}. Значит, сборка сделана \
             ДО поднятия версии, а манифест — после. Пересобери по порядку: \
             cargo build --release -> make-manifest.ps1 -Archive -> \
             make-setup.ps1 -> make-manifest.ps1"
        );
        // Номер сборки — тоже: он показывается в логах и в отчёте.
        let build = include_str!("app.rs")
            .lines()
            .find(|l| l.contains("const BUILD"))
            .and_then(|l| l.split('"').nth(1))
            .unwrap_or("");
        if !build.is_empty() {
            assert!(
                text.contains(build),
                "в собранном .exe нет номера сборки {build} — та же причина: \
                 сборка старше, чем правка исходников"
            );
        }
    }

    #[test]
    fn manifest_matches_the_file_actually_in_the_project_root() {
        // Вторая половина той же ловушки. Манифест — текст, версия в нём —
        // текст, они всегда согласованы между собой. Но он может быть
        // посчитан по файлу, который в корне уже не тот: скрипт манифеста
        // считал хеш от старой сборки, потому что новую в корень не
        // положили. Тогда программа скачает ровно то, что уже стоит.
        //
        // Если файла в корне нет (чистая выгрузка репозитория) — проверка
        // пропускается молча.
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let m = Manifest::parse(REAL_MANIFEST).unwrap();
        for f in &m.files {
            let p = root.join(&f.name);
            if !p.is_file() {
                eprintln!("{} нет в корне — проверка пропущена", f.name);
                continue;
            }
            let actual = sha256_file(&p)
                .unwrap_or_else(|e| panic!("не посчитан хеш {}: {e}", p.display()));
            assert_eq!(
                actual, f.sha256,
                "манифест описывает {name} с хешем {want}, а в корне лежит \
                 другой файл ({got}). Пересобери по порядку: \
                 cargo build --release -> make-manifest.ps1 -Archive -> \
                 make-setup.ps1 -> make-manifest.ps1",
                name = f.name,
                want = &f.sha256[..12],
                got = &actual[..12],
            );
        }
    }

    #[test]
    fn manifest_hashes_are_64_hex_chars() {
        let m = Manifest::parse(REAL_MANIFEST).unwrap();
        for f in &m.files {
            assert_eq!(
                f.sha256.len(),
                64,
                "{}: хеш должен быть 64 hex-символа SHA-256, а не {}",
                f.name,
                f.sha256.len()
            );
            assert!(
                f.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "{}: хеш содержит не-hex символы: {}",
                f.name,
                f.sha256
            );
        }
    }
}
