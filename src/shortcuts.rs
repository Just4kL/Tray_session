use std::collections::HashMap;
use std::fs;

const FILE: &str = "shortcuts.json";

/// Шорткат: модификаторы + клавиша, либо модификаторы + кнопка мыши.
/// Коды кнопок мыши: 0=Left, 1=Right, 2=Middle, 3=Extra1(Mouse4), 4=Extra2(Mouse5).
/// `rshift` = именно ПРАВЫЙ Shift (для RShift+стрелки и т.п.); обычный Shift — `shift`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub rshift: bool,
    pub key: Option<egui::Key>,
    pub mouse: Option<u8>,
}

pub struct ActionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub default: &'static str,
}

/// Все действия программы, которым можно назначить шорткат.
pub const ACTIONS: &[ActionDef] = &[
    ActionDef { id: "tab_sessions", label: "Вкладка «Сессии»", default: "Alt+1" },
    ActionDef { id: "tab_games", label: "Вкладка «Игры»", default: "Alt+2" },
    ActionDef { id: "tab_alarms", label: "Вкладка «Будильники»", default: "Alt+3" },
    ActionDef { id: "tab_timer", label: "Вкладка «Таймер»", default: "Alt+4" },
    ActionDef { id: "tab_about", label: "Вкладка «О программе»", default: "Alt+5" },
    ActionDef { id: "tab_shortcuts", label: "Вкладка «Параметры»", default: "Alt+6" },
    ActionDef { id: "quick_alarm", label: "Быстрый будильник (звук BEEP)", default: "Ctrl+}" },
    ActionDef { id: "quick_timer", label: "Быстрый таймер (минуты, Enter)", default: "Ctrl+{" },
    ActionDef { id: "export_csv", label: "Экспорт таблицы в CSV", default: "Ctrl+E" },
    ActionDef { id: "refresh", label: "Обновить таблицу сессий", default: "Ctrl+R" },
    ActionDef { id: "stopwatch_toggle", label: "Секундомер: старт/пауза", default: "RShift+Up" },
    ActionDef { id: "stopwatch_lap", label: "Секундомер: сохранить круг", default: "RShift+Right" },
    ActionDef { id: "stopwatch_stop", label: "Секундомер: стоп", default: "RShift+Left" },
    ActionDef { id: "stopwatch_pin", label: "Секундомер: закрепить/открепить", default: "RShift+Down" },
    ActionDef { id: "opacity_up", label: "Прозрачность закреплённого окна +5%", default: "Ctrl+Num+" },
    ActionDef { id: "opacity_down", label: "Прозрачность закреплённого окна −5%", default: "Ctrl+Num-" },
    // Стрелки на мини-трее: те же четыре комбинации RShift+стрелка, что и
    // у секундомера, но когда окошко секундомера закрыто, они управляют
    // полоской (позиция/прозрачность). Отдельные биндинги НЕ заводим:
    // RegisterHotKey не даёт дважды занять одну комбинацию, и вторая
    // регистрация молча провалилась бы. Маршрутизация — в poll_appcmd.
    ActionDef { id: "strip_pos_prev", label: "Мини-трей: позиция 3×3 назад (когда окошко секундомера закрыто)", default: "RShift+Left" },
    ActionDef { id: "strip_pos_next", label: "Мини-трей: позиция 3×3 вперёд (когда окошко секундомера закрыто)", default: "RShift+Right" },
    ActionDef { id: "strip_opacity_up", label: "Мини-трей: прозрачность +5% (когда окошко секундомера закрыто)", default: "RShift+Up" },
    ActionDef { id: "strip_opacity_down", label: "Мини-трей: прозрачность −5% (когда окошко секундомера закрыто)", default: "RShift+Down" },
    ActionDef { id: "paste_url", label: "Вставить URL из буфера (будильник/таймер)", default: "Ctrl+Shift+V" },
    ActionDef { id: "close_dialog", label: "Закрыть верхний диалог", default: "Esc" },
    ActionDef { id: "hide_tray", label: "Скрыть окно в трей", default: "Ctrl+H" },
];

pub fn action_label(id: &str) -> &str {
    ACTIONS.iter().find(|a| a.id == id).map(|a| a.label).unwrap_or(id)
}

fn action_default(id: &str) -> &str {
    ACTIONS.iter().find(|a| a.id == id).map(|a| a.default).unwrap_or("")
}

fn key_from_token(tok: &str) -> Option<egui::Key> {
    use egui::Key as K;
    if tok.len() == 1 {
        let c = tok.chars().next().unwrap().to_ascii_uppercase();
        if c.is_ascii_digit() {
            return match c {
                '0' => Some(K::Num0), '1' => Some(K::Num1), '2' => Some(K::Num2),
                '3' => Some(K::Num3), '4' => Some(K::Num4), '5' => Some(K::Num5),
                '6' => Some(K::Num6), '7' => Some(K::Num7), '8' => Some(K::Num8),
                _ => Some(K::Num9),
            };
        }
        if c.is_ascii_alphabetic() {
            return match c {
                'A' => Some(K::A), 'B' => Some(K::B), 'C' => Some(K::C),
                'D' => Some(K::D), 'E' => Some(K::E), 'F' => Some(K::F),
                'G' => Some(K::G), 'H' => Some(K::H), 'I' => Some(K::I),
                'J' => Some(K::J), 'K' => Some(K::K), 'L' => Some(K::L),
                'M' => Some(K::M), 'N' => Some(K::N), 'O' => Some(K::O),
                'P' => Some(K::P), 'Q' => Some(K::Q), 'R' => Some(K::R),
                'S' => Some(K::S), 'T' => Some(K::T), 'U' => Some(K::U),
                'V' => Some(K::V), 'W' => Some(K::W), 'X' => Some(K::X),
                'Y' => Some(K::Y), _ => Some(K::Z),
            };
        }
    }
    Some(match tok {
        "[" => K::OpenBracket,
        "]" => K::CloseBracket,
        ";" => K::Semicolon,
        "'" => K::Quote,
        "," => K::Comma,
        "." => K::Period,
        "/" => K::Slash,
        "\\" => K::Backslash,
        "`" => K::Backtick,
        "-" => K::Minus,
        "+" => K::Plus,
        "NUMPLUS" => K::Plus, // Num+ (numpad): глобально всегда VK_ADD
        "NUMMINUS" => K::Minus, // Num− (numpad): глобально всегда VK_SUBTRACT
        "=" => K::Equals,
        "ENTER" => K::Enter,
        "ESC" | "ESCAPE" => K::Escape,
        "SPACE" => K::Space,
        "TAB" => K::Tab,
        "BACKSPACE" => K::Backspace,
        "DELETE" => K::Delete,
        "INSERT" => K::Insert,
        "HOME" => K::Home,
        "END" => K::End,
        "PAGEUP" => K::PageUp,
        "PAGEDOWN" => K::PageDown,
        "UP" => K::ArrowUp,
        "DOWN" => K::ArrowDown,
        "LEFT" => K::ArrowLeft,
        "RIGHT" => K::ArrowRight,
        _ if tok.starts_with('F') && tok.len() <= 3 => {
            match tok[1..].parse::<u32>().ok() {
                Some(1) => K::F1, Some(2) => K::F2, Some(3) => K::F3,
                Some(4) => K::F4, Some(5) => K::F5, Some(6) => K::F6,
                Some(7) => K::F7, Some(8) => K::F8, Some(9) => K::F9,
                Some(10) => K::F10, Some(11) => K::F11, Some(12) => K::F12,
                _ => return None,
            }
        }
        _ => return None,
    })
}

fn mouse_from_token(tok: &str) -> Option<u8> {
    match tok {
        "MOUSELEFT" | "LEFTBUTTON" => Some(0),
        "MOUSERIGHT" | "RIGHTBUTTON" => Some(1),
        "MOUSEMIDDLE" | "MIDDLEBUTTON" => Some(2),
        "MOUSE4" | "EXTRA1" => Some(3),
        "MOUSE5" | "EXTRA2" => Some(4),
        _ => None,
    }
}

/// Разбор строки вида "Ctrl+Alt+E", "Alt+1", "Ctrl+{", "Ctrl+Mouse4".
/// "Num+"/"Num−" прячутся от split('+') в NUMPLUS/NUMMINUS.
pub fn parse_binding(s: &str) -> Option<Shortcut> {
    parse_binding_inner(&protect_numpad(s))
}

/// "Num+"/"Num-" содержат '+', ломающий split — прячем их.
fn protect_numpad(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if i + 3 < chars.len() && (chars[i] == 'N' || chars[i] == 'n')
            && (chars[i + 1] == 'U' || chars[i + 1] == 'u')
            && (chars[i + 2] == 'M' || chars[i + 2] == 'm')
            && (chars[i + 3] == '+' || chars[i + 3] == '-')
        {
            out.push_str(if chars[i + 3] == '+' { "NumPlus" } else { "NumMinus" });
            i += 4;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn parse_binding_inner(s: &str) -> Option<Shortcut> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Физически "{" = Ctrl+Shift+[, "}" = Ctrl+Shift+]
    if s.eq_ignore_ascii_case("ctrl+}") {
        return Some(Shortcut { ctrl: true, shift: true, alt: false, rshift: false, key: Some(egui::Key::CloseBracket), mouse: None });
    }
    if s.eq_ignore_ascii_case("ctrl+{") {
        return Some(Shortcut { ctrl: true, shift: true, alt: false, rshift: false, key: Some(egui::Key::OpenBracket), mouse: None });
    }
    let (mut ctrl, mut shift, mut alt, mut rshift) = (false, false, false, false);
    let mut main: Option<&str> = None;
    for part in s.split('+') {
        let t = part.trim().to_ascii_uppercase();
        match t.as_str() {
            "CTRL" | "CONTROL" => ctrl = true,
            "SHIFT" => shift = true,
            "RSHIFT" | "RIGHTSHIFT" | "RIGHT_SHIFT" => rshift = true,
            "ALT" => alt = true,
            "" => return None,
            _ => {
                if main.is_some() {
                    return None;
                }
                main = Some(part.trim());
            }
        }
    }
    let tok = main?.to_ascii_uppercase();
    if let Some(m) = mouse_from_token(&tok) {
        return Some(Shortcut { ctrl, shift, alt, rshift, key: None, mouse: Some(m) });
    }
    let key = key_from_token(&tok)?;
    Some(Shortcut { ctrl, shift, alt, rshift, key: Some(key), mouse: None })
}

fn key_token(k: egui::Key) -> &'static str {
    use egui::Key as K;
    match k {
        K::Num0 => "0", K::Num1 => "1", K::Num2 => "2", K::Num3 => "3",
        K::Num4 => "4", K::Num5 => "5", K::Num6 => "6", K::Num7 => "7",
        K::Num8 => "8", K::Num9 => "9",
        K::A => "A", K::B => "B", K::C => "C", K::D => "D", K::E => "E",
        K::F => "F", K::G => "G", K::H => "H", K::I => "I", K::J => "J",
        K::K => "K", K::L => "L", K::M => "M", K::N => "N", K::O => "O",
        K::P => "P", K::Q => "Q", K::R => "R", K::S => "S", K::T => "T",
        K::U => "U", K::V => "V", K::W => "W", K::X => "X", K::Y => "Y",
        K::Z => "Z",
        K::F1 => "F1", K::F2 => "F2", K::F3 => "F3", K::F4 => "F4",
        K::F5 => "F5", K::F6 => "F6", K::F7 => "F7", K::F8 => "F8",
        K::F9 => "F9", K::F10 => "F10", K::F11 => "F11", K::F12 => "F12",
        K::Enter => "Enter", K::Escape => "Esc", K::Space => "Space",
        K::Tab => "Tab", K::Backspace => "Backspace", K::Delete => "Delete",
        K::Insert => "Insert", K::Home => "Home", K::End => "End",
        K::PageUp => "PageUp", K::PageDown => "PageDown",
        K::ArrowUp => "Up", K::ArrowDown => "Down",
        K::ArrowLeft => "Left", K::ArrowRight => "Right",
        K::OpenBracket => "[", K::CloseBracket => "]",
        K::Semicolon => ";", K::Quote => "'", K::Comma => ",",
        K::Period => ".", K::Slash => "/", K::Backslash => "\\",
        K::Backtick => "`", K::Minus => "Num-", K::Equals => "=",
        K::Plus => "Num+",        _ => "?",
    }
}

fn mouse_token(m: u8) -> &'static str {
    match m {
        0 => "MouseLeft",
        1 => "MouseRight",
        2 => "MouseMiddle",
        3 => "Mouse4",
        _ => "Mouse5",
    }
}

pub fn mouse_button(m: u8) -> Option<egui::PointerButton> {
    use egui::PointerButton as P;
    match m {
        0 => Some(P::Primary),
        1 => Some(P::Secondary),
        2 => Some(P::Middle),
        3 => Some(P::Extra1),
        _ => Some(P::Extra2),
    }
}

/// Win32 virtual-key код для egui-клавиши (глобальные хоткеи RegisterHotKey).
/// Неподдерживаемые (Copy/Cut/Paste и т.п.) -> None.
pub fn key_to_vk(k: egui::Key) -> Option<u32> {
    use egui::Key as K;
    Some(match k {
        K::A => 0x41, K::B => 0x42, K::C => 0x43, K::D => 0x44,
        K::E => 0x45, K::F => 0x46, K::G => 0x47, K::H => 0x48,
        K::I => 0x49, K::J => 0x4A, K::K => 0x4B, K::L => 0x4C,
        K::M => 0x4D, K::N => 0x4E, K::O => 0x4F, K::P => 0x50,
        K::Q => 0x51, K::R => 0x52, K::S => 0x53, K::T => 0x54,
        K::U => 0x55, K::V => 0x56, K::W => 0x57, K::X => 0x58,
        K::Y => 0x59, K::Z => 0x5A,
        K::Num0 => 0x30, K::Num1 => 0x31, K::Num2 => 0x32,
        K::Num3 => 0x33, K::Num4 => 0x34, K::Num5 => 0x35,
        K::Num6 => 0x36, K::Num7 => 0x37, K::Num8 => 0x38,
        K::Num9 => 0x39,
        K::F1 => 0x70, K::F2 => 0x71, K::F3 => 0x72, K::F4 => 0x73,
        K::F5 => 0x74, K::F6 => 0x75, K::F7 => 0x76, K::F8 => 0x77,
        K::F9 => 0x78, K::F10 => 0x79, K::F11 => 0x7A, K::F12 => 0x7B,
        K::Enter => 0x0D, K::Escape => 0x1B, K::Space => 0x20,
        K::Tab => 0x09, K::Backspace => 0x08, K::Delete => 0x2E,
        K::Insert => 0x2D, K::Home => 0x24, K::End => 0x23,
        K::PageUp => 0x21, K::PageDown => 0x22,
        K::ArrowUp => 0x26, K::ArrowDown => 0x28,
        K::ArrowLeft => 0x25, K::ArrowRight => 0x27,
        K::OpenBracket => 0xDB, K::CloseBracket => 0xDD,
        K::Semicolon => 0xBA, K::Quote => 0xDE, K::Comma => 0xBC,
        K::Period => 0xBE, K::Slash => 0xBF, K::Backslash => 0xDC,
        K::Backtick => 0xC0, K::Minus => 0x6D, K::Equals => 0xBB,
        // Plus/Minus трактуем как Num+/Num− (numpad): это требование
        // шорткатов прозрачности (см. parse_binding "NUM+"/"NUM-").
        K::Plus => 0x6B,
        _ => return None,
    })
}

/// Глобальная регистрация хоткея для потока трея (RegisterHotKey).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyReg {
    pub uid: u32,
    pub action: String,
    pub mods: u32, // Win32 MOD_* (1=Alt, 2=Ctrl, 4=Shift)
    pub vk: u32,
    /// Только правый Shift (проверяется стороной при срабатывании).
    pub rshift_only: bool,
}

/// Нажат ли сейчас правый Shift (а левый — нет). Только Windows.
#[cfg(windows)]
pub fn rshift_down() -> bool {
    use winapi::um::winuser::{GetAsyncKeyState, VK_LSHIFT, VK_RSHIFT};
    unsafe { GetAsyncKeyState(VK_RSHIFT) < 0 && GetAsyncKeyState(VK_LSHIFT) >= 0 }
}

/// Нажат ли сейчас левый Shift. Только Windows.
#[cfg(windows)]
pub fn lshift_down() -> bool {
    use winapi::um::winuser::{GetAsyncKeyState, VK_LSHIFT};
    unsafe { GetAsyncKeyState(VK_LSHIFT) < 0 }
}

#[cfg(not(windows))]
pub fn rshift_down() -> bool {
    false
}

#[cfg(not(windows))]
pub fn lshift_down() -> bool {
    false
}

/// Человекочитаемый текст шортката. Сохраняет привычные "Ctrl+{" / "Ctrl+}".
pub fn display(s: &Shortcut) -> String {
    use egui::Key as K;
    if s.key == Some(K::OpenBracket) && s.ctrl && s.shift && !s.alt && s.mouse.is_none() {
        return "Ctrl+{".to_string();
    }
    if s.key == Some(K::CloseBracket) && s.ctrl && s.shift && !s.alt && s.mouse.is_none() {
        return "Ctrl+}".to_string();
    }
    let mut out = String::new();
    if s.ctrl {
        out += "Ctrl+";
    }
    if s.alt {
        out += "Alt+";
    }
    if s.rshift {
        out += "RShift+";
    } else if s.shift {
        out += "Shift+";
    }
    if let Some(m) = s.mouse {
        out += mouse_token(m);
    } else if let Some(k) = s.key {
        out += key_token(k);
    } else {
        out += "—";
    }
    out
}

/// Клавиши, которые можно захватить при переназначении.
pub const CAPTURE_KEYS: &[egui::Key] = &[
    egui::Key::A, egui::Key::B, egui::Key::C, egui::Key::D, egui::Key::E,
    egui::Key::F, egui::Key::G, egui::Key::H, egui::Key::I, egui::Key::J,
    egui::Key::K, egui::Key::L, egui::Key::M, egui::Key::N, egui::Key::O,
    egui::Key::P, egui::Key::Q, egui::Key::R, egui::Key::S, egui::Key::T,
    egui::Key::U, egui::Key::V, egui::Key::W, egui::Key::X, egui::Key::Y,
    egui::Key::Z,
    egui::Key::Num0, egui::Key::Num1, egui::Key::Num2, egui::Key::Num3,
    egui::Key::Num4, egui::Key::Num5, egui::Key::Num6, egui::Key::Num7,
    egui::Key::Num8, egui::Key::Num9,
    egui::Key::F1, egui::Key::F2, egui::Key::F3, egui::Key::F4,
    egui::Key::F5, egui::Key::F6, egui::Key::F7, egui::Key::F8,
    egui::Key::F9, egui::Key::F10, egui::Key::F11, egui::Key::F12,
    egui::Key::Enter, egui::Key::Space, egui::Key::Tab,
    egui::Key::Backspace, egui::Key::Delete, egui::Key::Insert,
    egui::Key::Home, egui::Key::End, egui::Key::PageUp, egui::Key::PageDown,
    egui::Key::ArrowUp, egui::Key::ArrowDown,
    egui::Key::ArrowLeft, egui::Key::ArrowRight,
    egui::Key::OpenBracket, egui::Key::CloseBracket,
    egui::Key::Semicolon, egui::Key::Quote, egui::Key::Comma,
    egui::Key::Period, egui::Key::Slash, egui::Key::Backslash,
    egui::Key::Backtick, egui::Key::Minus, egui::Key::Equals,
    egui::Key::Plus,];

/// Переопределения пользователя: id -> строка биндинга. Сохраняется само.
pub struct ShortcutStore {
    map: HashMap<String, String>,
}

impl ShortcutStore {
    pub fn load() -> Self {
        Self::load_from(FILE)
    }

    pub fn load_from(path: &str) -> Self {
        let map = fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<HashMap<String, String>>(&t).ok())
            .unwrap_or_default();
        // Отбрасываем мусор, оставляем только известные действия с валидными строками
        let map = map
            .into_iter()
            .filter(|(k, v)| ACTIONS.iter().any(|a| a.id == k.as_str()) && parse_binding(v).is_some())
            .collect();
        Self { map }
    }

    pub fn save(&self) {
        self.save_to(FILE);
    }

    pub fn save_to(&self, path: &str) {
        let _ = fs::write(path, serde_json::to_string_pretty(&self.map).unwrap_or_default());
    }

    /// Текущий биндинг действия (переопределение или умолчание).
    pub fn binding(&self, id: &str) -> Shortcut {
        self.map
            .get(id)
            .and_then(|s| parse_binding(s))
            .or_else(|| parse_binding(action_default(id)))
            .unwrap_or(Shortcut { ctrl: false, shift: false, alt: false, rshift: false, key: None, mouse: None })
    }

    pub fn binding_str(&self, id: &str) -> String {
        display(&self.binding(id))
    }

    pub fn is_custom(&self, id: &str) -> bool {
        self.map.contains_key(id)
    }

    pub fn set(&mut self, id: &str, s: Shortcut) {
        self.map.insert(id.to_string(), display(&s));
        self.save();
    }

    pub fn reset(&mut self, id: &str) {
        self.map.remove(id);
        self.save();
    }

    pub fn reset_all(&mut self) {
        self.map.clear();
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defaults_parse() {
        for a in ACTIONS {
            assert!(parse_binding(a.default).is_some(), "не парсится {}", a.default);
        }
    }

    #[test]
    fn display_parse_roundtrip() {
        for s in ["Alt+1", "Ctrl+E", "Ctrl+R", "Ctrl+H", "Ctrl+{", "Ctrl+}", "Esc",
                  "Alt+Shift+F5", "Ctrl+Alt+Delete", "Ctrl+Mouse4", "Shift+Space", "F12", "["] {
            let b = parse_binding(s).unwrap_or_else(|| panic!("не парсится {s}"));
            let shown = display(&b);
            let b2 = parse_binding(&shown).unwrap_or_else(|| panic!("не парсится {shown}"));
            assert_eq!(b, b2, "{s} -> {shown}");
        }
    }

    #[test]
    fn brace_bindings_have_shift() {
        // "{" физически = Shift+[, "}" = Shift+]
        let open = parse_binding("Ctrl+{").unwrap();
        assert!(open.ctrl && open.shift && !open.alt);
        assert_eq!(open.key, Some(egui::Key::OpenBracket));
        assert_eq!(display(&open), "Ctrl+{");
        let close = parse_binding("Ctrl+}").unwrap();
        assert_eq!(close.key, Some(egui::Key::CloseBracket));
        assert_eq!(display(&close), "Ctrl+}");
    }

    #[test]
    fn strip_hotkeys_do_not_collide_with_stopwatch() {
        // Секундомер и мини-трей делят комбинации RShift+стрелка, поэтому
        // одновременно зарегистрировать оба набора нельзя: RegisterHotKey
        // вернёт ошибку на вторую регистрацию. Проверяем, что дефолты у
        // соответствующих действий совпадают по комбинации (маршрутизация
        // выбирается в TrackerApp::push_stopwatch_hotkeys), и что пересечение
        // между наборами ровно предсказуемое.
        let get = |id: &str| parse_binding(action_default(id)).expect("дефолт не разбирается");
        // Пары «одна комбинация — одно действиние» в каждом наборе.
        for (a, b) in [
            ("stopwatch_toggle", "strip_opacity_up"),
            ("stopwatch_pin", "strip_opacity_down"),
            ("stopwatch_stop", "strip_pos_prev"),
            ("stopwatch_lap", "strip_pos_next"),
        ] {
            let (x, y) = (get(a), get(b));
            assert_eq!(
                (x.key, x.ctrl, x.shift, x.alt, x.rshift),
                (y.key, y.ctrl, y.shift, y.alt, y.rshift),
                "{a} и {b} должны быть на одной комбинации (конфликт RegisterHotKey)"
            );
            // И это именно стрелки RShift — ими управляют окошки.
            assert!(x.rshift, "{a} должен быть на правом Shift");
            assert!(
                matches!(
                    x.key,
                    Some(egui::Key::ArrowUp)
                        | Some(egui::Key::ArrowDown)
                        | Some(egui::Key::ArrowLeft)
                        | Some(egui::Key::ArrowRight)
                ),
                "{a} должен быть на стрелке, а не на {:?}",
                x.key
            );
        }
        // Внутри набора мини-трея все четыре комбинации разные.
        let strip: Vec<_> = ["strip_opacity_up", "strip_opacity_down", "strip_pos_prev", "strip_pos_next"]
            .iter()
            .map(|id| format!("{}", display(&get(id))))
            .collect();
        let mut uniq = strip.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), 4, "дубли в наборе мини-трея: {strip:?}");
    }

    #[test]
    fn rshift_bindings() {
        let up = parse_binding("RShift+Up").unwrap();
        assert!(up.rshift && !up.shift && !up.ctrl && !up.alt);
        assert_eq!(up.key, Some(egui::Key::ArrowUp));
        assert_eq!(display(&up), "RShift+Up");
        let down = parse_binding("rshift+down").unwrap();
        assert_eq!(display(&down), "RShift+Down");
        // Обычный Shift не превращается в RShift
        let s = parse_binding("Ctrl+Shift+X").unwrap();
        assert!(s.shift && !s.rshift);
    }

    #[test]
    fn vk_mapping() {
        assert_eq!(key_to_vk(egui::Key::A), Some(0x41));
        assert_eq!(key_to_vk(egui::Key::Z), Some(0x5A));
        assert_eq!(key_to_vk(egui::Key::Num1), Some(0x31));
        assert_eq!(key_to_vk(egui::Key::F5), Some(0x74));
        assert_eq!(key_to_vk(egui::Key::ArrowUp), Some(0x26));
        assert_eq!(key_to_vk(egui::Key::ArrowLeft), Some(0x25));
        assert_eq!(key_to_vk(egui::Key::Enter), Some(0x0D));
        assert_eq!(key_to_vk(egui::Key::OpenBracket), Some(0xDB));
        assert_eq!(key_to_vk(egui::Key::Copy), None);
    }

    #[test]
    fn invalid_rejected() {
        for s in ["", "   ", "Ctrl+", "Foo", "Ctrl+E+X", "Alt++E", "Ctrl+Mouse9"] {
            assert!(parse_binding(s).is_none(), "должно отклоняться: {s}");
        }
    }

    #[test]
    fn store_roundtrip_and_filtering() {
        let path = std::env::temp_dir().join("tray_session_shortcuts_test.json");
        let path = path.to_string_lossy().to_string();
        let _ = std::fs::remove_file(&path);
        let mut st = ShortcutStore::load_from(&path);
        assert_eq!(st.binding_str("tab_sessions"), "Alt+1");
        st.set("tab_sessions", parse_binding("Ctrl+Alt+1").unwrap());
        assert!(st.is_custom("tab_sessions"));
        assert_eq!(st.binding_str("tab_sessions"), "Ctrl+Alt+1");
        // Мусор в файле отбрасывается
        std::fs::write(&path, r#"{"tab_sessions": "Foo", "unknown_action": "Ctrl+X"}"#).unwrap();
        let st2 = ShortcutStore::load_from(&path);
        assert!(!st2.is_custom("tab_sessions"));
        assert_eq!(st2.binding_str("tab_sessions"), "Alt+1");
        st.reset("tab_sessions");
        assert!(!st.is_custom("tab_sessions"));
        let _ = std::fs::remove_file(&path);
    }
}
