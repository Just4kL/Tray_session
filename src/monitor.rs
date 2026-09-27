use crate::config::{AppConfig, TrackedGame};
use crate::detector::{build_match_maps, ProcInfo};
use crate::gpu::{query_gpu, vram_mb};
use chrono::Local;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, RwLock};
use std::time::Duration;

// События монитора -> UI-поток (UI пишет в БД и показывает уведомления).
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    Started { game_name: String, exe_path: String },
    Ended { exe_key: String },
    ActiveTick,
}

#[derive(Debug, Clone)]
pub struct ActiveInfo {
    pub game_name: String,
    pub exe_path: String,
    pub exe_key: String,
    pub start: chrono::DateTime<Local>,
    pub session_id: Option<i64>,
    pub cpu: f32,
    pub mem_mb: u64,
    pub vram_mb: u64,
    pub gpu: bool,
    pub pending: bool, // ещё не подтверждена (анти-дребезг), время не пишем
}

pub type SharedGames = Arc<RwLock<Vec<TrackedGame>>>;
pub type SharedActive = Arc<RwLock<HashMap<String, ActiveInfo>>>;
pub type SharedConfig = Arc<RwLock<AppConfig>>;

struct PendingState {
    hits: u32,
}

pub fn spawn_monitor(
    games: SharedGames,
    active: SharedActive,
    cfg: SharedConfig,
    tx: mpsc::Sender<MonitorEvent>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use sysinfo::System;
        let mut sys = System::new_all();
        // Прогрев для корректного cpu_usage
        sys.refresh_processes();
        sys.refresh_cpu();
        // missing_since для grace period + hits для confirm
        let mut missing: HashMap<String, chrono::DateTime<Local>> = HashMap::new();
        let mut pending: HashMap<String, PendingState> = HashMap::new();

        loop {
            let (interval, grace, require_gpu, min_vram, min_cpu, min_ram, confirm) = {
                let c = cfg.read().unwrap();
                (
                    c.check_interval_secs.max(2),
                    c.grace_secs.max(2),
                    c.require_gpu,
                    c.min_vram_mb,
                    c.min_cpu_pct,
                    c.min_ram_mb,
                    c.confirm_hits.max(1),
                )
            };

            sys.refresh_processes();
            sys.refresh_cpu();
            // sysinfo 0.30: cpu считается между вызовами refresh; делаем короткую паузу
            // раз в цикл, чтобы значения были живыми, но цикл оставался ~interval.
            let gpu = query_gpu();

            // Снимок процессов
            let mut current: HashMap<String, ProcInfo> = HashMap::new();
            let mut pid_by_key: HashMap<String, u32> = HashMap::new();
            // Карты совпадений
            let games_guard = games.read().unwrap();
            let (by_path, by_file) = build_match_maps(&games_guard);
            for p in sys.processes().values() {
                let exe = p.exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
                if exe.is_empty() {
                    continue;
                }
                if let Some(g) = crate::detector::match_tracked(&exe, &by_path, &by_file) {
                    let key = g.key();
                    // Если один exe запущен несколько раз — берём экземпляр с большим CPU
                    let info = ProcInfo {
                        pid: p.pid().as_u32(),
                        name: p.name().to_string(),
                        exe: exe.clone(),
                        cpu: p.cpu_usage(),
                        mem_mb: p.memory() / 1024,
                    };
                    let replace = match current.get(&key) {
                        Some(old) => info.cpu > old.cpu,
                        None => true,
                    };
                    if replace {
                        pid_by_key.insert(key.clone(), info.pid);
                        current.insert(key, info);
                    }
                }
            }
            drop(games_guard);

            let now = Local::now();

            // 1) Обработка появлений / активности
            // Клонируем ключи, чтобы не держать lock
            let tracked_keys: Vec<(String, String)> = {
                let g = games.read().unwrap();
                g.iter().map(|t| (t.key(), t.name.clone())).collect()
            };
            for (key, game_name) in tracked_keys {
                let proc_opt = current.get(&key);
                // GPU-гейт — ключевая логика против ложных срабатываний лаунчера:
                // процесс считается ИГРОЙ только если его PID виден в NVML.
                // На Linux проверяем ещё и VRAM выше порога; на Windows (WDDM)
                // драйвер байты не отдаёт — доверяем факту присутствия в списке.
                let (is_active, cpu, mem, vram, on_gpu) = match proc_opt {
                    Some(proc) => {
                        let vram_bytes = gpu.vram_by_pid.get(&proc.pid).copied().unwrap_or(0);
                        let vram = vram_mb(vram_bytes);
                        let on_gpu = gpu.pids.contains(&proc.pid);
                        let vram_unknown = gpu.vram_unknown.contains(&proc.pid);
                        let active = if gpu.usable && require_gpu {
                            // Строгий режим.
                            // Лаунчер (Steam, EGS, HoYoPlay) обычно висит в процессах,
                            // но в GPU-списках его нет — сессия не стартует.
                            if !on_gpu {
                                false
                            } else if vram_unknown {
                                true // Windows: факта рендера достаточно
                            } else {
                                vram >= min_vram
                            }
                        } else {
                            // Fallback без NVML (AMD/Intel): CPU или RAM выше порога.
                            proc.cpu >= min_cpu || proc.mem_mb >= min_ram
                        };
                        (active, proc.cpu, proc.mem_mb, vram, on_gpu)
                    }
                    None => (false, 0.0, 0, 0, false),
                };

                let mut act = active.write().unwrap();
                if is_active {
                    missing.remove(&key);
                    if let Some(a) = act.get_mut(&key) {
                        // Уже отслеживается: обновить метрики, снять pending если подтвердилась
                        a.cpu = cpu;
                        a.mem_mb = mem;
                        a.vram_mb = vram;
                        a.gpu = on_gpu;
                        if a.pending {
                            // подтверждаем сразу, т.к. hits уже были набраны при создании
                            a.pending = false;
                        }
                    } else {
                        // Кандидат: набираем hits
                        let hits = pending.get(&key).map(|p| p.hits).unwrap_or(0) + 1;
                        if hits >= confirm {
                            // Подтверждено — стартуем сессию
                            pending.remove(&key);
                            act.insert(
                                key.clone(),
                                ActiveInfo {
                                    game_name: game_name.clone(),
                                    exe_path: proc_opt.map(|p| p.exe.clone()).unwrap_or_default(),
                                    exe_key: key.clone(),
                                    start: now,
                                    session_id: None,
                                    cpu,
                                    mem_mb: mem,
                                    vram_mb: vram,
                                    gpu: on_gpu,
                                    pending: false,
                                },
                            );
                            drop(act);
                            let _ = tx.send(MonitorEvent::Started {
                                game_name,
                                exe_path: key.clone(),
                            });
                            let _ = tx.send(MonitorEvent::ActiveTick);
                            continue;
                        } else {
                            pending.insert(key.clone(), PendingState { hits });
                            // Показываем как "проверка..." чтобы было видно, что гейт работает
                            act.insert(
                                key.clone(),
                                ActiveInfo {
                                    game_name: game_name.clone(),
                                    exe_path: proc_opt.map(|p| p.exe.clone()).unwrap_or_default(),
                                    exe_key: key.clone(),
                                    start: now,
                                    session_id: None,
                                    cpu,
                                    mem_mb: mem,
                                    vram_mb: vram,
                                    gpu: on_gpu,
                                    pending: true,
                                },
                            );
                        }
                    }
                } else {
                    // Процесс неактивен: либо его нет, либо это лаунчер без GPU.
                    // Если была неподтверждённая запись (pending) — убираем её тихо.
                    if let Some(a) = act.get(&key) {
                        if a.pending {
                            act.remove(&key);
                            pending.remove(&key);
                            drop(act);
                            let _ = tx.send(MonitorEvent::ActiveTick);
                            continue;
                        }
                    }
                    pending.remove(&key);
                    // Если была активная сессия — запускаем grace period
                    if act.contains_key(&key) {
                        let since = missing.entry(key.clone()).or_insert(now);
                        let elapsed = (now - *since).num_seconds();
                        if elapsed >= grace as i64 {
                            act.remove(&key);
                            missing.remove(&key);
                            drop(act);
                            let _ = tx.send(MonitorEvent::Ended { exe_key: key.clone() });
                            let _ = tx.send(MonitorEvent::ActiveTick);
                            continue;
                        }
                    } else {
                        missing.remove(&key);
                    }
                }
                drop(act);
            }

            // 2) Чистим active от игр, удалённых из отслеживания
            {
                let keys: Vec<String> = games.read().unwrap().iter().map(|g| g.key()).collect();
                let mut act = active.write().unwrap();
                let stale: Vec<String> = act.keys().filter(|k| !keys.contains(k)).cloned().collect();
                for k in stale {
                    act.remove(&k);
                }
            }

            std::thread::sleep(Duration::from_secs(interval));
        }
    })
}
