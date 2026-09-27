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

    fn spawn_beep(&mut self, kind: win_beep::Kind, secs: u64) {
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
                win_beep::beep(kind);
                for _ in 0..20 {
                    if flag.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }));
    }

    pub fn play_standard(&mut self, name: &str, secs: u64) {
        match name {
            "bell" => self.spawn_beep(win_beep::Kind::Asterisk, secs),
            _ => self.spawn_beep(win_beep::Kind::Exclamation, secs),
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
    #[derive(Clone, Copy)]
    pub enum Kind {
        Exclamation,
        Asterisk,
    }
    pub fn beep(kind: Kind) {
        use winapi::um::winuser::{MessageBeep, MB_ICONASTERISK, MB_ICONEXCLAMATION};
        unsafe {
            match kind {
                Kind::Exclamation => { MessageBeep(MB_ICONEXCLAMATION); }
                Kind::Asterisk => { MessageBeep(MB_ICONASTERISK); }
            }
        }
    }
}

#[cfg(not(windows))]
mod win_beep {
    #[derive(Clone, Copy)]
    pub enum Kind {
        Exclamation,
        Asterisk,
    }
    pub fn beep(_kind: Kind) {
        print!("\x07");
    }
}
