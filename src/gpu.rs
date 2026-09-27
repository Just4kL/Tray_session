use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default)]
pub struct GpuSnapshot {
    pub usable: bool,
    /// % загрузки GPU (max по картам), 0 если недоступно
    pub gpu_util_pct: u32,
    /// PID, замеченные в graphics/compute списках NVML
    pub pids: HashSet<u32>,
    /// VRAM в байтах (0 если драйвер не отдал значение — Windows WDDM)
    pub vram_by_pid: HashMap<u32, u64>,
    /// PID, для которых драйвер вернул Unavailable (Windows) — доверяем факту on_gpu
    pub vram_unknown: HashSet<u32>,
}

static NVML_OK: OnceLock<bool> = OnceLock::new();

// Хэндл NVML кэшируется на поток: раньше Nvml::init() дёргался при каждом
// опросе (раз в ~10 с + раз в 5 с из UI), теперь инициализация одна на поток.
thread_local! {
    static NVML_HANDLE: RefCell<Option<nvml_wrapper::Nvml>> = const { RefCell::new(None) };
}

fn nvml_available() -> bool {
    *NVML_OK.get_or_init(|| {
        match nvml_wrapper::Nvml::init() {
            Ok(nvml) => {
                let _ = nvml.device_count();
                true
            }
            Err(_) => false,
        }
    })
}

/// Опрашивает NVML: какие PID реально грузят видеокарту и сколько VRAM занимают.
/// Если NVML недоступен (AMD/Intel/нет драйвера) — usable=false и вызывающий
/// код должен использовать fallback по CPU/RAM.
pub fn query_gpu() -> GpuSnapshot {
    let mut snap = GpuSnapshot::default();
    if !nvml_available() {
        return snap;
    }
    NVML_HANDLE.with(|h| {
        let mut h = h.borrow_mut();
        if h.is_none() {
            *h = nvml_wrapper::Nvml::init().ok();
        }
        let nvml = match h.as_ref() {
            Some(n) => n,
            None => return,
        };
        let count = match nvml.device_count() {
            Ok(c) => c,
            Err(_) => return,
        };
        if count == 0 {
            return;
        }
        snap.usable = true; // драйвер есть — снимок валиден даже если список пуст
        for i in 0..count {
            let dev = match nvml.device_by_index(i) {
                Ok(d) => d,
                Err(_) => continue,
            };
            if let Ok(rates) = dev.utilization_rates() {
                snap.gpu_util_pct = snap.gpu_util_pct.max(rates.gpu);
            }
            // Graphics-процессы (игры) + Compute (некоторые движки идут как compute)
            let mut procs = Vec::new();
            if let Ok(v) = dev.running_graphics_processes() {
                procs.extend(v);
            }
            if let Ok(v) = dev.running_compute_processes() {
                procs.extend(v);
            }
            for p in procs {
                snap.pids.insert(p.pid);
                match p.used_gpu_memory {
                    nvml_wrapper::enums::device::UsedGpuMemory::Used(v) => {
                        let entry = snap.vram_by_pid.entry(p.pid).or_insert(0);
                        *entry = (*entry).max(v);
                    }
                    nvml_wrapper::enums::device::UsedGpuMemory::Unavailable => {
                        // Windows WDDM: байты недоступны, но сам факт присутствия
                        // PID в списке = процесс рендерит на NVIDIA GPU.
                        snap.vram_unknown.insert(p.pid);
                        snap.vram_by_pid.entry(p.pid).or_insert(0);
                    }
                }
            }
        }
    });
    snap
}

pub fn vram_mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}
