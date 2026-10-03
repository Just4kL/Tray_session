use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Простой воспроизводитель «стандартных» звуков:
/// повторяющийся системный beep до 3 минут + открытие файлов/URL через `open`.
pub struct SoundPlayer {
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self { stop_flag: Arc::new(AtomicBool::new(false)), worker: None }
    }
}

impl SoundPlayer {
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    fn spawn_beep(&mut self, freq_hz: u32, tone_ms: u64, secs: u64) {
        self.stop();
        let flag = Arc::new(AtomicBool::new(false));
        self.stop_flag = flag.clone();
        let secs = secs.min(180);
        let repeats = (secs / 2).max(1);
        self.worker = Some(std::thread::spawn(move || {
            for _ in 0..repeats {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                win_beep::tone(freq_hz, tone_ms);
                for _ in 0..20 {
                    if flag.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }));
    }

    /// T-4: Beep и Bell — гарантированно разные тона (частота задана
    /// явно). Раньше оба шли через MessageBeep, который на многих
    /// системах маппится на один и тот же wav.
    pub fn play_standard(&mut self, name: &str, secs: u64) {
        match name {
            "bell" => self.spawn_beep(523, 400, secs),
            _ => self.spawn_beep(880, 250, secs),
        }
    }

    pub fn play_file(path: &str) {
        let _ = open::that(path);
    }
    pub fn open_url(url: &str) {
        let _ = open::that(url);
    }
}

#[cfg(windows)]
mod win_beep {
    /// Один тон заданной частоты/длительности. Детерминированно отличается
    /// от других тонов (в отличие от MessageBeep, зависящего от настроек ОС).
    pub fn tone(freq_hz: u32, dur_ms: u64) {
        use winapi::um::utilapiset::Beep;
        unsafe {
            Beep(freq_hz, dur_ms.min(u32::MAX as u64) as u32);
        }
    }
}

#[cfg(not(windows))]
mod win_beep {
    pub fn tone(_freq_hz: u32, dur_ms: u64) {
        print!("\x07");
        std::thread::sleep(std::time::Duration::from_millis(dur_ms.min(2000)));
    }
}
