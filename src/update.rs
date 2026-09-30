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

/// Ключ, которым фоновый процесс просит основной программу закрыться.
///
/// Основной процесс пишет этот файл, когда пользователь согласился на
/// обновление, и у себя же проверяет его в цикле отрисовки. Обновление
/// ждёт файл недолго и, если основной процесс не закрылся, отменяется —
/// заменять запущенный файл нельзя.
pub const READY_FLAG: &str = "update_ready.flag";

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
    let c = client(20)?;
    let text = c
        .get(&manifest_url_for(channel))
        .send()
        .map_err(|e| format!("не скачался манифест: {e}"))?
        .error_for_status()
        .map_err(|e| format!("манифест недоступен: {e}"))?
        .text()
        .map_err(|e| format!("манифест не прочитан: {e}"))?;
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
pub fn download_to(url: &str, dest: &Path, expect_sha: &str) -> Result<u64, String> {
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match try_download_once(url, dest, expect_sha, attempt) {
            Ok(n) => return Ok(n),
            Err(e) => {
                last_err = e;
                if attempt < MAX_ATTEMPTS {
                    let delay = std::time::Duration::from_secs(retry_delay_secs(attempt));
                    crate::log::warn(&format!(
                        "попытка {attempt}/{MAX_ATTEMPTS} скачать {url} не удалась: {last_err}. Повтор через {delay:?}"
                    ));
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

/// Что нужно сделать с файлами: заменить, удалить лишнее, ничего.
#[derive(Debug, Clone, PartialEq)]
pub enum ApplyResult {
    /// Файлы заменены (перечислены новые хеши, уже применённые — нет).
    Replaced(Vec<String>),
    /// Заменять нечего: всё уже новое.
    NothingToDo,
}

/// Заменить файлы программы на скачанные.
///
/// Возвращает список ПРОВЕРЕННЫХ файлов — по нему основной процесс
/// понимает, какие из них действительно поменялись, а какие были уже
/// такими же (обновлять было нечего).
///
/// Сначала читаем размеры, потом заменяем. Важно: сравнение идёт по
/// размеру на диске, а не по хешу из манифеста. Иначе updater, скачавший
/// файл самой программы, посчитал бы его изменившимся (хеш манифеста
/// отличается от хеша только что скачанного, ведь программа теперь на диске
/// и есть этот файл) и заменил бы файл на его же копию.
pub fn apply_update(
    base: &Path,
    tmp: &Path,
    files: &[FileEntry],
) -> Result<ApplyResult, String> {
    // 1. Собираем размеры новых файлов и заодно решаем, что менять.
    let mut to_replace: Vec<(String, u64)> = Vec::new();
    for f in files {
        if is_protected(&f.name) || !is_safe_rel_name(&f.name) {
            continue;
        }
        let src = tmp.join(&f.name);
        let Ok(meta) = std::fs::metadata(&src) else { continue };
        if !meta.is_file() {
            continue;
        }
        let dst = base.join(&f.name);
        let same = dst
            .metadata()
            .map(|m| m.is_file() && m.len() == meta.len())
            .unwrap_or(false);
        if !same {
            to_replace.push((f.name.clone(), meta.len()));
        }
    }
    if to_replace.is_empty() {
        return Ok(ApplyResult::NothingToDo);
    }
    // 2. Меняем. Windows не даёт заменить файл, который сейчас запущен,
    //    поэтому основной процесс к этому моменту уже должен закрыться.
    for (name, _) in &to_replace {
        let src = tmp.join(name);
        let dst = base.join(name);
        std::fs::copy(&src, &dst)
            .map_err(|e| format!("не заменён {name}: {e}"))?;
    }
    Ok(ApplyResult::Replaced(
        to_replace.into_iter().map(|(n, _)| n).collect(),
    ))
}

// ---------------------------------------------------------------------------
// Фоновый процесс
// ---------------------------------------------------------------------------

/// Прочитать аргументы командной строки: запущен ли updater и с какими
/// параметрами.
pub fn parse_args(args: &[String]) -> Option<UpdateArgs> {
    if !args.iter().any(|a| a == UPDATER_FLAG) {
        return None;
    }
    Some(UpdateArgs {
        wait_secs: args
            .iter()
            .position(|a| a == UPDATER_FLAG)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(20),
        // Канал идёт третьим аргументом после числа секунд. Передавать
        // его нужно обязательно: фоновый процесс — отдельный, и без
        // явного канала он обновил бы программу по стабильной ветке.
        channel: args
            .iter()
            .position(|a| a == UPDATER_FLAG)
            .and_then(|i| args.get(i + 2))
            .cloned()
            .unwrap_or_else(|| CHANNEL_STABLE.to_string()),
    })
}

/// Флаг, которым основная программа запускает сама себя как updater.
pub const UPDATER_FLAG: &str = "--updater";

/// Параметры фонового процесса.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateArgs {
    /// Сколько секунд ждать, пока основная программа закроется.
    pub wait_secs: u64,
    /// Канал обновлений: `stable` или `beta`.
    pub channel: String,
}

/// Имя, под которым фоновый процесс прячет сам себя на время работы.
pub const UPDATER_NAME: &str = "_updater_running.exe";

/// Убрать себя с пути, освободив имя `TraySession.exe` под новую сборку.
///
/// Windows не позволяет ЗАМЕНИТЬ файл, который сейчас выполняется, но
/// позволяет его ПЕРЕИМЕНОВАТЬ. Без этого фоновый процесс не смог бы
/// обновить самого себя: он и есть тот файл, который требуется перезаписать
/// (ошибка «процесс не может получить доступ к файлу», os error 32).
///
/// Возвращает путь, под которым процесс теперь работает, — его надо удалить
/// в конце.
pub fn self_rename_aside(base: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("не найден свой exe: {e}"))?;
    rename_aside_from(&exe, base)
}

/// Переименовать `exe` в `update_tmp/_updater_running.exe`.
///
/// Вынесено отдельно от `self_rename_aside`, чтобы можно было проверить
/// переименование на обычном файле, не трогая реальный исполняемый файл
/// процесса.
pub fn rename_aside_from(exe: &Path, base: &Path) -> Result<PathBuf, String> {
    // Уже переименован (повторный запуск) — тогда инициализировать нечего.
    if exe.file_name().map(|n| n == UPDATER_NAME).unwrap_or(false) {
        return Ok(exe.to_path_buf());
    }
    let aside = tmp_dir(base).join(UPDATER_NAME);
    if let Some(d) = aside.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("не создана папка {}: {e}", d.display()))?;
    }
    std::fs::rename(exe, &aside).map_err(|e| {
        format!(
            "не удалось убрать себя с пути ({} -> {}): {e}",
            exe.display(),
            aside.display()
        )
    })?;
    Ok(aside)
}

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

/// Точка входа фонового процесса.
///
/// Тело вынесено отдельно, чтобы переименованная копия себя удалялась на
/// ЛЮБОМ выходе — и при ошибке, и при отсутствии обновлений. Иначе
/// `_updater_running.exe` остался бы лежать в папке обновлений.
pub fn run_updater(wait_secs: u64, channel: &str) -> i32 {
    let base = program_dir();
    crate::log::init(&base);
    let (code, aside) = run_updater_inner(&base, wait_secs, channel);
    if !aside.as_os_str().is_empty() {
        finish_aside(&base, &aside);
    }
    code
}

/// Что сделать с переименованной копией программы в конце работы.
///
/// Копию удаляем, только если новая сборка уже встала на своё место. Иначе
/// переименованный файл — это и есть программа, и удаление оставляло бы
/// человека вообще без программы: отменённое обновление, обрыв загрузки,
/// нехватка прав — всё это стирало TraySession.exe безвозвратно.
///
/// Решение принимается по состоянию диска, а не по коду возврата: код
/// отражает, на каком шаге остановились, а файл — на что реально можно
/// опереться.
pub fn finish_aside(base: &Path, aside: &Path) -> AsideOutcome {
    let main = base.join(MAIN_EXE);
    if main.is_file() {
        let _ = remove_aside_delayed(aside);
        return AsideOutcome::Deleted;
    }
    // Программы на месте нет — значит она всё ещё лежит под новым именем.
    match std::fs::rename(aside, &main) {
        Ok(()) => {
            crate::log::info("старая программа возвращена на место");
            AsideOutcome::Restored
        }
        Err(e) => {
            crate::log::err(&format!(
                "программа пропала: не смог вернуть {} на место: {e}",
                aside.display()
            ));
            AsideOutcome::Lost
        }
    }
}

/// Что `finish_aside` сделал с копией.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsideOutcome {
    /// Новая сборка встала на место, копия удалена — так и должно быть.
    Deleted,
    /// Обновление не вышло, копия возвращена под именем программы.
    Restored,
    /// Вернуть не удалось: копия осталась под служебным именем, и её надо
    /// переименовать вручную.
    Lost,
}

/// Основная работа обновления. Возвращает код и путь, под которым процесс
/// себя переименовал (пустой, если не смог).
fn run_updater_inner(base: &Path, wait_secs: u64, channel: &str) -> (i32, PathBuf) {
    let base = base.to_path_buf();
    // C14: второй апдейтер. Два параллельных процесса делили один
    // TraySession.exe (rename_aside_from) и один update_report.json.
    // Имя отличается от C5-мутекса main-окна: main + updater обязаны
    // сосуществовать, C5 намеренно пропускает --updater.
    #[cfg(windows)]
    let _updater_mutex = {
        use winapi::shared::winerror::ERROR_ALREADY_EXISTS;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::synchapi::CreateMutexW;
        let name: Vec<u16> = "Local\\TraySession_Updater\0".encode_utf16().collect();
        unsafe {
            let h = CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
            if GetLastError() == ERROR_ALREADY_EXISTS {
                crate::log::warn("второй апдейтер уже запущен, выходим");
                let _ = write_report(
                    &base,
                    UpdateReport {
                        ok: false,
                        message: "обновление уже выполняется".to_string(),
                        ..Default::default()
                    },
                );
                std::process::exit(0);
            }
            h
        }
    };
    crate::log::info(&format!(
        "фоновый процесс обновления запущен, канал {}, ждём закрытия программы до {wait_secs} с",
        channel_label(channel)
    ));
    // Первым делом уходим с имени TraySession.exe: иначе нельзя будет
    // заменить сам этот файл, пока мы его выполняем (os error 32).
    let aside = match self_rename_aside(&base) {
        Ok(p) => {
            crate::log::info(&format!(
                "освободил имя программы: работаю под именем {}",
                p.display()
            ));
            p
        }
        Err(e) => {
            // Не смертельно: если exe не в use (например, запустили копию),
            // замена может пройти и без переименования. Пробуем дальше.
            crate::log::warn(&format!("переименовать себя не вышло: {e}"));
            PathBuf::new()
        }
    };
    let tmp = tmp_dir(&base);
    if let Err(e) = std::fs::create_dir_all(&tmp) {
        crate::log::err(&format!("не создана папка загрузки {}: {e}", tmp.display()));
        let _ = write_report(
            &base,
            UpdateReport {
                ok: false,
                message: format!("не создана папка загрузки {}: {e}", tmp.display()),
                ..Default::default()
            },
        );
        return (2, aside.clone());
    }
    // Отчёт всегда свежий: старый мог остаться от прошлого обновления.
    UpdateReport::clear(&base);
    UpdateReport {
        ok: false,
        message: "не удалось получить манифест обновления".to_string(),
        ..Default::default()
    }
    .save(&base)
    .ok();

    // 1. Манифест.
    crate::log::info(&format!(
        "запрашиваю манифест: {}",
        manifest_url_for(channel)
    ));
    let manifest = match fetch_manifest_for(channel) {
        Ok(m) => {
            crate::log::info(&format!(
                "манифест получен: версия {}, сборка {}, файлов: {}",
                m.version,
                m.build,
                m.files.len()
            ));
            m
        }
        Err(e) => {
            crate::log::err(&format!("манифест не получен: {e}"));
            let _ = write_report(&base, UpdateReport { ok: false, message: e, ..Default::default() });
            return (3, aside.clone());
        }
    };
    // 2. План: какие файлы отличаются от диска.
    let plan = plan_update(&base, &manifest);
    crate::log::info(&format!(
        "план: скачать {}, совпадают {}, данных пользователя рядом: {}",
        plan.to_download.len(),
        plan.unchanged.len(),
        plan.preserved.len()
    ));
    for f in &plan.to_download {
        crate::log::info(&format!("  к замене: {}", f.name));
    }
    for f in &plan.unchanged {
        crate::log::info(&format!("  без изменений: {f}"));
    }
    for f in &plan.preserved {
        crate::log::info(&format!("  НЕ ТРОГАЕМ (данные пользователя): {f}"));
    }
    let mut report = UpdateReport {
        preserved: plan.preserved.clone(),
        ..Default::default()
    };
    if plan.to_download.is_empty() {
        report.ok = true;
        report.unchanged = plan.unchanged.clone();
        report.message = format!("уже установлена версия {}", manifest.version);
        crate::log::info("обновлять нечего: файлы совпадают с манифестом");
        let _ = write_report(&base, report);
        return (0, aside.clone());
    }
    // 3. Скачать в папку рядом с программой.
    for f in &plan.to_download {
        let url = file_url_for(channel, &f.name);
        crate::log::info(&format!("качаю {url}"));
        if let Err(e) = download_to(&url, &tmp.join(&f.name), &f.sha256) {
            crate::log::err(&format!("загрузка не удалась: {e}"));
            let _ = write_report(
                &base,
                UpdateReport {
                    ok: false,
                    message: e,
                    preserved: plan.preserved.clone(),
                    ..Default::default()
                },
            );
            return (4, aside.clone());
        }
        crate::log::info(&format!("скачано и сверено по SHA-256: {}", f.name));
    }
    // 4. Дождаться, пока основная программа закроется. Она сама закроется,
    //    когда увидит файл-флаг; если не закрылась — не трогаем файлы.
    crate::log::info("жду сигнала закрытия от основной программы");
    if !wait_for_exit(&base, wait_secs) {
        crate::log::err("сигнала закрытия не было: файлы не тронуты");
        let _ = write_report(
            &base,
            UpdateReport {
                ok: false,
                message: "программа не закрылась, файлы не тронуты".to_string(),
                preserved: plan.preserved.clone(),
                ..Default::default()
            },
        );
        return (5, aside.clone());
    }
    crate::log::info("сигнал получен, заменяю файлы");
    // 5. Заменить файлы.
    match apply_update(&base, &tmp, &plan.to_download) {
        Err(e) => {
            crate::log::err(&format!("замена не удалась: {e}"));
            let _ = write_report(
                &base,
                UpdateReport {
                    ok: false,
                    message: e,
                    preserved: plan.preserved.clone(),
                    ..Default::default()
                },
            );
            return (6, aside.clone());
        }
        Ok(ApplyResult::NothingToDo) => {
            report.ok = true;
            report.unchanged = plan.unchanged.clone();
            report.message = format!("файлы уже обновлены до {}", manifest.version);
            crate::log::info("файлы оказались уже новыми");
            let _ = write_report(&base, report);
        }
        Ok(ApplyResult::Replaced(updated)) => {
            report.ok = true;
            // Логируем до переноса: `updated` уходит в move строкой ниже.
            crate::log::info(&format!("заменено файлов: {}", updated.join(", ")));
            report.updated = updated;
            report.unchanged = plan.unchanged.clone();
            report.message = format!("установлена версия {}", manifest.version);
            // 6. Перезапустить программу.
            let exe = base.join(MAIN_EXE);
            match std::process::Command::new(&exe).spawn() {
                Ok(_) => crate::log::info(&format!("программа перезапущена: {}", exe.display())),
                Err(e) => {
                    // Раньше ошибка перезапуска глоталась: файлы обновлены,
                    // а человек думал, что всё прошло. Теперь это попадает и
                    // в лог, и в отчёт.
                    crate::log::err(&format!(
                        "не удалось перезапустить программу ({e}); запустите {} вручную",
                        exe.display()
                    ));
                    report.message.push_str(&format!(
                        ". Файлы обновлены, но перезапуск не удался: {e}. Запустите {} вручную",
                        exe.display()
                    ));
                }
            }
            let _ = write_report(&base, report);
        }
    }
    crate::log::info("обновление завершено");
    (0, aside)
}

fn write_report(base: &Path, r: UpdateReport) -> std::io::Result<()> {
    r.save(base)
}

/// Ждать сигнала от основной программы: файл `update_ready.flag`.
///
/// Основная программа пишет его перед выходом. Пока файла нет, ждём;
/// если за отведённое время не появился — обновление отменяется, чтобы не
/// бить по работающей программе.
pub fn wait_for_exit(base: &Path, wait_secs: u64) -> bool {
    let flag = base.join(READY_FLAG);
    let start = std::time::Instant::now();
    // Флаг проверяется ХОТЯ БЫ ОДИН РАЗ, даже если ждать нечего. Иначе
    // `wait_secs = 0` означал бы «не смотреть вовсе», и программа, которая
    // уже успела записать файл и выйти, считалась бы ещё работающей:
    // обновление отменялось бы, а её файл так и лежал бы дальше.
    loop {
        if flag.exists() {
            let _ = std::fs::remove_file(&flag);
            // Дать основному процессу дописать и выйти.
            std::thread::sleep(std::time::Duration::from_millis(600));
            return true;
        }
        if start.elapsed().as_secs() >= wait_secs {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// Попросить основную программу закрыться: записать файл-флаг.
pub fn signal_ready_to_exit(base: &Path) -> std::io::Result<()> {
    std::fs::write(base.join(READY_FLAG), b"1")
}

/// Убрать остаточный файл-флаг при старте программы.
///
/// Флаг живёт рядом с программой и переживает её: если она упала или была
/// убита в момент установки, файл остаётся. А `wait_for_exit` проверяет его
/// на первой же итерации и, увидев, разрешает замену файлов немедленно — то
/// ест�� ровно тот случай, когда заменять нельзя. Поэтому при старте флаг
/// снимается: он нужен только в текущем сеансе.
///
/// Вызывается один раз при запуске главной программы.
pub fn clear_stale_ready_flag(base: &Path) -> bool {
    let flag = base.join(READY_FLAG);
    if flag.exists() {
        std::fs::remove_file(&flag).is_ok()
    } else {
        false
    }
}

/// Запустить фоновый процесс обновления.
///
/// `exe` — путь к своему исполняемому файлу. Отдельный процесс нужен
/// затем, что (а) интерфейс не должен висеть на загрузке, (б) заменять
/// `.exe` может только тот, кто его не держит открытым.
pub fn spawn_updater(exe: &Path, wait_secs: u64, channel: &str) -> std::io::Result<()> {
    std::process::Command::new(exe)
        .arg(UPDATER_FLAG)
        .arg(wait_secs.to_string())
        // Канал обязателен: без него фоновый процесс взял бы стабильную
        // ветку, и бета-сборка молча откатывалась бы на старую версию.
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
    /// apply_update() его не трогает. Файл на диске после всего этого
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
        // «Старая» программа и «новая» в папке загрузки.
        std::fs::write(d.join("TraySession.exe"), b"OLD").unwrap();
        let tmp = d.join("update_tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("TraySession.exe"), b"NEW-BINARY-0123456789").unwrap();
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
        apply_update(d, &tmp, &plan.to_download).unwrap();
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
