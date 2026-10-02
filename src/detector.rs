use crate::config::{exe_file_name, normalize_exe, TrackedGame};
use crate::gpu::bytes_to_mb;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub cpu: f32,
    pub mem_mb: u64,
}

// ---------- Steam paths ----------

#[cfg(windows)]
pub fn steam_install_path() -> PathBuf {
    if let Ok(hkcu) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam") {
        if let Ok(p) = hkcu.get_value::<String, _>("SteamPath") {
            let pb = PathBuf::from(&p);
            if pb.exists() {
                return pb;
            }
        }
    }
    if let Ok(hklm) = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Valve\Steam") {
        if let Ok(p) = hklm.get_value::<String, _>("InstallPath") {
            let pb = PathBuf::from(&p);
            if pb.exists() {
                return pb;
            }
        }
    }
    PathBuf::from(r"C:\Program Files (x86)\Steam")
}

#[cfg(not(windows))]
pub fn steam_install_path() -> PathBuf {
    PathBuf::from(r"C:\Program Files (x86)\Steam")
}

pub fn library_folders(steam_path: &Path) -> Vec<PathBuf> {
    let mut libs = Vec::new();
    let vdf = steam_path.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(&vdf) {
        // ищем "path" "..."
        let mut search_from = 0;
        while let Some(idx) = content[search_from..].find("\"path\"") {
            let abs = search_from + idx + 6;
            if let Some(q1) = content[abs..].find('"') {
                let s = abs + q1 + 1;
                if let Some(q2) = content[s..].find('"') {
                    let raw = &content[s..s + q2];
                    let fixed = raw.replace("\\\\", "\\");
                    libs.push(PathBuf::from(&fixed).join("steamapps"));
                    search_from = s + q2 + 1;
                    continue;
                }
            }
            break;
        }
    }
    let def = steam_path.join("steamapps");
    if !libs.iter().any(|p| p == &def) {
        libs.push(def);
    }
    libs.into_iter().filter(|p| p.exists()).collect()
}

pub fn local_steam_id(steam_path: &Path) -> Option<String> {
    let vdf = steam_path.join("config").join("loginusers.vdf");
    let content = fs::read_to_string(vdf).ok()?;
    // ищем 17-значные id перед " {"
    let bytes = content.as_bytes();
    let mut out: Option<String> = None;
    let mut i = 0;
    while i + 17 <= bytes.len() {
        if bytes[i].is_ascii_digit() && content[i..i + 17].chars().all(|c| c.is_ascii_digit()) {
            let rest = &content[i + 17..];
            let rest_trim = rest.trim_start();
            if rest_trim.starts_with('{') || rest_trim.starts_with('"') {
                out = Some(content[i..i + 17].to_string());
                break;
            }
            i += 17;
        } else {
            i += 1;
        }
    }
    out
}

// ---------- Steam API ----------

#[derive(Debug)]
pub struct OwnedGame {
    pub appid: u32,
    pub name: String,
    /// Всего минут по данным серверов Steam (playtime_forever).
    /// Наши сессии замеряют дельты и прибавляются к этому общему.
    pub playtime_forever: u32,
}

pub fn get_owned_games(api_key: &str, steam_id: &str) -> Result<Vec<OwnedGame>, String> {
    let url = format!(
        "http://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/?key={}&steamid={}&format=json&include_appinfo=1",
        api_key.trim(),
        steam_id.trim()
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().map_err(|e| format!("Сетевая ошибка: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP ошибка {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().map_err(|_| "Некорректный ответ Steam API".to_string())?;
    let games = v
        .get("response")
        .and_then(|r| r.get("games"))
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for g in games {
        out.push(OwnedGame {
            appid: g.get("appid").and_then(|a| a.as_u64()).unwrap_or(0) as u32,
            name: g.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
            playtime_forever: g.get("playtime_forever").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
        });
    }
    Ok(out)
}

fn find_install_dir(manifest: &Path) -> Option<String> {
    let content = fs::read_to_string(manifest).ok()?;
    // "installdir" "..."
    let idx = content.find("\"installdir\"")?;
    let after = &content[idx + 13..];
    let q1 = after.find('"')?;
    let s = q1 + 1;
    let q2 = after[s..].find('"')?;
    Some(after[s..s + q2].to_string())
}

fn biggest_exe_in_dir(game_path: &Path, max_depth: usize) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    let mut stack: Vec<(PathBuf, usize)> = vec![(game_path.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let entries = fs::read_dir(&dir).ok()?;
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push((p, depth + 1));
            } else if p.extension().map(|x| x.to_string_lossy().to_lowercase() == "exe").unwrap_or(false) {
                let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                // Отсеиваем явные установщики/редисты/лаунчеры (проблема Python-версии:
                // Social-Club-Setup.exe, VC_redist и т.п.; из живого прогона:
                // NeedForSpeedHeatTrial.exe, start_protected_game.exe) —
                // штрафом, а не исключением: если кроме лаунчера ничего нет,
                // возьмём его.
                let lower = file_name.clone();
                let is_redist = lower.contains("redist")
                    || lower.contains("vcredist")
                    || lower.contains("dotnet")
                    || lower.contains("directx")
                    || lower.contains("unins")
                    || lower.contains("uninstall")
                    || lower.contains("setup")
                    || lower.contains("installer")
                    || lower.contains("crashhandler")
                    || lower.contains("crash_reporter")
                    || lower.contains("launcher")
                    || lower.contains("start_protected")
                    || lower.contains("trial")
                    || lower.contains("eac")
                    || lower.contains("easyanticheat");
                let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                // Штраф для редистов: делим размер, чтобы настоящий exe побеждал
                let score = if is_redist { size / 100 } else { size };
                if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                    best = Some((score, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

pub fn scan_steam_games(steam_path: Option<PathBuf>) -> Vec<TrackedGame> {
    let sp = steam_path.unwrap_or_else(steam_install_path);
    let libs = library_folders(&sp);
    let mut games = Vec::new();
    for lib in libs {
        let entries = match fs::read_dir(&lib) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            if !(fname.starts_with("appmanifest_") && fname.ends_with(".acf")) {
                continue;
            }
            let manifest = e.path();
            let content = match fs::read_to_string(&manifest) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // name + installdir
            let name = {
                let mut n: Option<String> = None;
                if let Some(idx) = content.find("\"name\"") {
                    let after = &content[idx + 6..];
                    if let Some(q1) = after.find('"') {
                        let s = q1 + 1;
                        if let Some(q2) = after[s..].find('"') {
                            n = Some(after[s..s + q2].to_string());
                        }
                    }
                }
                n
            };
            let installdir = find_install_dir(&manifest);
            if let (Some(name), Some(dir)) = (name, installdir) {
                let game_path = lib.join("common").join(&dir);
                if game_path.exists() {
                    if let Some(exe) = biggest_exe_in_dir(&game_path, 3) {
                        games.push(TrackedGame {
                            name,
                            exe_path: exe.to_string_lossy().to_string(),
                            source: "Steam".to_string(),
                            steam_minutes: None,
                        });
                    }
                }
            }
        }
    }
    games
}

pub fn scan_steam_games_via_api(api_key: &str, steam_id: &str, steam_path: Option<PathBuf>) -> Result<Vec<TrackedGame>, String> {
    let sp = steam_path.unwrap_or_else(steam_install_path);
    let libs = library_folders(&sp);
    let owned = get_owned_games(api_key, steam_id)?;
    let mut games = Vec::new();
    for g in owned {
        let mut manifest: Option<PathBuf> = None;
        for lib in &libs {
            let m = lib.join(format!("appmanifest_{}.acf", g.appid));
            if m.exists() {
                manifest = Some(m);
                break;
            }
        }
        let manifest = match manifest {
            Some(m) => m,
            None => continue,
        };
        let installdir = match find_install_dir(&manifest) {
            Some(d) => d,
            None => continue,
        };
        let game_path = manifest.parent().unwrap().join("common").join(installdir);
        if !game_path.exists() {
            continue;
        }
        if let Some(exe) = biggest_exe_in_dir(&game_path, 3) {
            games.push(TrackedGame {
                name: g.name,
                exe_path: exe.to_string_lossy().to_string(),
                source: "SteamAPI".to_string(),
                steam_minutes: Some(g.playtime_forever as u64),
            });
        }
    }
    Ok(games)
}

// ---------- Windows registry ----------

#[cfg(windows)]
pub fn scan_windows_games() -> Vec<TrackedGame> {
    use winreg::enums::*;
    let mut games = Vec::new();
    let publisher_kw = [
        "game", "entertainment", "interactive", "software", "studio", "ubisoft",
        "electronic arts", "bethesda", "activision", "blizzard", "epic games",
        "gog", "steam", "valve", "rockstar", "sega", "square enix", "capcom",
        "konami", "bandai namco", "cd projekt", "warner bros", "sony",
        "microsoft", "xbox game studios", "hoyo", "mihoyo", "riot",
    ];
    let install_kw = [
        "steamapps", "common", "epic games", "gog games", "origin games",
        "ubisoft", "games", "gamedata",
    ];
    let roots = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];
    for root in roots {
        for hkey in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            let key = match winreg::RegKey::predef(hkey).open_subkey(root) {
                Ok(k) => k,
                Err(_) => continue,
            };
            for sub in key.enum_keys().flatten() {
                let sk = match key.open_subkey(&sub) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
                let display_name: String = sk.get_value("DisplayName").unwrap_or_default();
                let display_icon: String = sk.get_value("DisplayIcon").unwrap_or_default();
                let install_loc: String = sk.get_value("InstallLocation").unwrap_or_default();
                let publisher: String = sk.get_value("Publisher").unwrap_or_default();
                if display_icon.is_empty() {
                    continue;
                }
                // DisplayIcon может быть "path,0" — отрезаем запятую
                let mut exe = display_icon.split(',').next().unwrap_or("").trim().trim_matches('"').to_string();
                if exe.is_empty() || !exe.to_lowercase().ends_with(".exe") {
                    continue;
                }
                if !Path::new(&exe).exists() {
                    continue;
                }
                let pub_l = publisher.to_lowercase();
                let loc_l = install_loc.to_lowercase();
                let name_l = display_name.to_lowercase();
                let mut is_game = publisher_kw.iter().any(|k| pub_l.contains(k));
                if !is_game {
                    is_game = install_kw.iter().any(|k| loc_l.contains(k));
                }
                if !is_game {
                    is_game = name_l.contains("game") || name_l.contains("игра");
                }
                if is_game {
                    let nm = if display_name.is_empty() {
                        Path::new(&exe).file_stem().and_then(|s| s.to_str()).unwrap_or("game").to_string()
                    } else {
                        display_name
                    };
                    // Нормализуем двойные слеши из реестра
                    exe = exe.replace("\\\\", "\\");
                    games.push(TrackedGame { name: nm, exe_path: exe, source: "WindowsRegistry".to_string(), steam_minutes: None });
                }
            }
        }
    }
    games
}

#[cfg(not(windows))]
pub fn scan_windows_games() -> Vec<TrackedGame> {
    Vec::new()
}

// ---------- Папка с игрой (указана пользователем) ----------

pub fn scan_folder_for_exes(folder: &Path, max_depth: usize) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(folder.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push((p, depth + 1));
            } else if p.extension().map(|x| x.to_string_lossy().to_lowercase() == "exe").unwrap_or(false) {
                let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                out.push((p, size));
            }
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}

// ---------- Активные процессы (для выбора пользователем) ----------

pub fn list_running_processes() -> Vec<ProcInfo> {
    // Один проход без пауз: раньше было два refresh_processes() со sleep(250мс),
    // что замораживало UI на четверть секунды при каждом открытии/обновлении
    // списка. CPU-метрики при одиночном проходе приблизительные (sysinfo
    // считает CPU между опросами) — для выбора программы из списка достаточно,
    // сортировка идёт по памяти.
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_processes();
    sys.refresh_cpu();
    let mut v: Vec<ProcInfo> = sys
        .processes()
        .values()
        .map(|p| {
            let exe = p.exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            ProcInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string(),
                exe,
                cpu: p.cpu_usage(),
                mem_mb: bytes_to_mb(p.memory()),
            }
        })
        .collect();
    v.sort_by(|a, b| b.mem_mb.cmp(&a.mem_mb));
    v
}

/// Сопоставление запущенного процесса с отслеживаемой игрой.
/// 1) точное совпадение нормализованного пути;
/// 2) совпадение только по имени файла (покрывает переезд библиотеки на другой диск).
pub fn match_tracked<'a>(proc_exe: &str, by_path: &'a HashMap<String, TrackedGame>, by_file: &'a HashMap<String, Vec<TrackedGame>>) -> Option<&'a TrackedGame> {
    if proc_exe.is_empty() {
        return None;
    }
    let norm = normalize_exe(proc_exe);
    if let Some(g) = by_path.get(&norm) {
        return Some(g);
    }
    let f = exe_file_name(proc_exe);
    if f.is_empty() {
        return None;
    }
    by_file.get(&f).and_then(|v| v.first())
}

pub fn build_match_maps(games: &[TrackedGame]) -> (HashMap<String, TrackedGame>, HashMap<String, Vec<TrackedGame>>) {
    let mut by_path = HashMap::new();
    let mut by_file: HashMap<String, Vec<TrackedGame>> = HashMap::new();
    for g in games {
        by_path.insert(g.key(), g.clone());
        by_file.entry(g.file_name()).or_default().push(g.clone());
    }
    (by_path, by_file)
}

#[cfg(test)]
mod live_tests {    /// Живой отчёт Steam-скана (только вручную: нужен установленный Steam).
    /// cargo test -- --ignored --nocapture live_steam_scan_report
    #[test]
    #[ignore]
    fn live_steam_scan_report() {
        let sp = super::steam_install_path();
        println!("steam path: {}", sp.display());
        for l in super::library_folders(&sp) {
            println!("lib: {}", l.display());
        }
        let t0 = std::time::Instant::now();
        let games = super::scan_steam_games(None);
        println!("found {} games in {:?}", games.len(), t0.elapsed());
        for g in games.iter().take(15) {
            println!("  [{}] {} -> {}", g.source, g.name, g.exe_path);
        }
    }
}
