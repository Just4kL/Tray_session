use chrono::{DateTime, Local};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

use crate::config::game_day_key;

#[derive(Debug, Clone)]
pub struct AggRow {
    pub game_name: String,
    pub exe_path: String,
    pub total_duration: i64,
    pub last_start: String,
    pub runs: i64,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: i64,
    pub game_name: String,
    pub exe_path: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AlarmRow {
    pub id: i64,
    pub time: String, // "HH:MM"
    pub enabled: bool,
    pub repeat_daily: bool,
    pub url: Option<String>,
    pub sound_file: Option<String>,
    pub standard_sound: Option<String>,
    /// Маска дней "1111111" (Пн..Вс). None/пустая = разовый будильник.
    /// Старые строки без days трактуются по repeat_daily.
    pub days: Option<String>,
}

pub struct Db {
    inner: Mutex<Connection>,
    pub day_start_hour: Mutex<u32>,
}

impl Db {
    pub fn open<P: AsRef<Path>>(path: P, day_start_hour: u32) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let db = Self { inner: Mutex::new(conn), day_start_hour: Mutex::new(day_start_hour) };
        db.init()?;
        Ok(db)
    }

    pub fn day_start(&self) -> u32 {
        *self.day_start_hour.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_day_start_hour(&self, h: u32) {
        if let Ok(mut v) = self.day_start_hour.lock() {
            *v = h;
        }
    }

    fn init(&self) -> Result<(), String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_name TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                start_time TEXT NOT NULL,
                end_time TEXT,
                duration_seconds INTEGER,
                date_key TEXT
            );
            CREATE TABLE IF NOT EXISTS alarms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                time TEXT NOT NULL,
                enabled INTEGER DEFAULT 1,
                repeat_daily INTEGER DEFAULT 0,
                url TEXT,
                sound_file TEXT,
                standard_sound TEXT
            );",
        )
        .map_err(|e| e.to_string())?;
        // Миграция alarms
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(alarms)")
            .map_err(|e| e.to_string())?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for (col, ddl) in [
            ("url", "ALTER TABLE alarms ADD COLUMN url TEXT"),
            ("sound_file", "ALTER TABLE alarms ADD COLUMN sound_file TEXT"),
            ("standard_sound", "ALTER TABLE alarms ADD COLUMN standard_sound TEXT"),
            // Маска дней недели "1111111" (Пн..Вс). NULL/"" = разовый будильник.
            ("days", "ALTER TABLE alarms ADD COLUMN days TEXT"),
        ] {
            if !cols.iter().any(|c| c == col) {
                let _ = conn.execute(ddl, []);
            }
        }
        Ok(())
    }

    pub fn add_session(&self, game_name: &str, exe_path: &str, start: &DateTime<Local>) -> Result<i64, String> {
        let key = game_day_key(start, self.day_start());
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO sessions (game_name, exe_path, start_time, date_key) VALUES (?1, ?2, ?3, ?4)",
            params![game_name, exe_path, start.to_rfc3339(), key],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn end_session(&self, session_id: i64, end: &DateTime<Local>) -> Result<i64, String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        let start_str: Option<String> = conn
            .query_row("SELECT start_time FROM sessions WHERE id = ?1", params![session_id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if let Some(s) = start_str {
            let start = DateTime::parse_from_rfc3339(&s)
                .map(|d| d.with_timezone(&Local))
                .unwrap_or(*end);
            let dur = (*end - start).num_seconds().max(0);
            conn.execute(
                "UPDATE sessions SET end_time = ?1, duration_seconds = ?2 WHERE id = ?3",
                params![end.to_rfc3339(), dur, session_id],
            )
            .map_err(|e| e.to_string())?;
            Ok(dur)
        } else {
            Ok(0)
        }
    }

    pub fn sessions_for_day(&self, day_key: &str) -> Vec<(String, i64)> {
        let conn = match self.inner.lock() { Ok(c) => c, Err(_) => return vec![] };
        let mut stmt = match conn.prepare(
            "SELECT game_name, duration_seconds FROM sessions WHERE date_key = ?1 AND duration_seconds IS NOT NULL",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![day_key], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn aggregated(&self) -> Vec<AggRow> {
        let conn = match self.inner.lock() { Ok(c) => c, Err(_) => return vec![] };
        let mut stmt = match conn.prepare(
            "SELECT game_name, exe_path, SUM(duration_seconds), MAX(start_time), COUNT(*)
             FROM sessions WHERE duration_seconds IS NOT NULL
             GROUP BY game_name ORDER BY SUM(duration_seconds) DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| {
            Ok(AggRow {
                game_name: r.get(0)?,
                exe_path: r.get(1)?,
                total_duration: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                last_start: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                runs: r.get::<_, i64>(4)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn sessions_by_game(&self, game_name: &str) -> Vec<SessionRow> {
        let conn = match self.inner.lock() { Ok(c) => c, Err(_) => return vec![] };
        let mut stmt = match conn.prepare(
            "SELECT id, game_name, exe_path, start_time, end_time, duration_seconds
             FROM sessions WHERE game_name = ?1 ORDER BY start_time ASC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![game_name], |r| {
            Ok(SessionRow {
                id: r.get(0)?,
                game_name: r.get(1)?,
                exe_path: r.get(2)?,
                start_time: r.get(3)?,
                end_time: r.get(4)?,
                duration: r.get(5)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn total_all(&self) -> i64 {
        let conn = match self.inner.lock() { Ok(c) => c, Err(_) => return 0 };
        conn.query_row(
            "SELECT SUM(duration_seconds) FROM sessions WHERE duration_seconds IS NOT NULL",
            [],
            |r| r.get::<_, Option<i64>>(0),
        )
        .unwrap_or(None)
        .unwrap_or(0)
    }

    pub fn update_session_duration(&self, id: i64, new_dur: i64, new_end: &DateTime<Local>) -> Result<(), String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE sessions SET duration_seconds = ?1, end_time = ?2 WHERE id = ?3",
            params![new_dur, new_end.to_rfc3339(), id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_session(&self, id: i64) -> Result<(), String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Последняя завершённая сессия: (игра, длительность, время конца).
    pub fn last_finished_session(&self) -> Option<(String, i64, String)> {
        let conn = self.inner.lock().ok()?;
        conn.query_row(
            "SELECT game_name, duration_seconds, end_time FROM sessions
             WHERE duration_seconds IS NOT NULL AND end_time IS NOT NULL
             ORDER BY end_time DESC LIMIT 1",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)),
        )
        .ok()
    }

    // ---- alarms ----
    pub fn add_alarm(&self, time: &str, daily: bool, url: Option<&str>, sound: Option<&str>, std_sound: Option<&str>, days: Option<&str>) -> Result<(), String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO alarms (time, repeat_daily, url, sound_file, standard_sound, days) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![time, if daily { 1 } else { 0 }, url, sound, std_sound, days],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn alarms(&self) -> Vec<AlarmRow> {
        let conn = match self.inner.lock() { Ok(c) => c, Err(_) => return vec![] };
        let mut stmt = match conn.prepare("SELECT id, time, enabled, repeat_daily, url, sound_file, standard_sound, days FROM alarms ORDER BY time") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| {
            Ok(AlarmRow {
                id: r.get(0)?,
                time: r.get(1)?,
                enabled: r.get::<_, i64>(2)? != 0,
                repeat_daily: r.get::<_, i64>(3)? != 0,
                url: r.get(4)?,
                sound_file: r.get(5)?,
                standard_sound: r.get(6)?,
                days: r.get(7)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn set_alarm_enabled(&self, id: i64, enabled: bool) -> Result<(), String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE alarms SET enabled = ?1 WHERE id = ?2", params![if enabled { 1 } else { 0 }, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_alarm_time(&self, id: i64, t: &str) -> Result<(), String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE alarms SET time = ?1 WHERE id = ?2", params![t, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_alarm_days(&self, id: i64, days: Option<&str>) -> Result<(), String> {
        let daily = matches!(days, Some("1111111")) as i32;
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE alarms SET days = ?1, repeat_daily = ?2 WHERE id = ?3", params![days, daily, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_alarm(&self, id: i64) -> Result<(), String> {
        let conn = self.inner.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM alarms WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
