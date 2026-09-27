//! Проверка обновлений. Этап 1: только проверка соединения с GitHub.
//! Следующий этап: сбор информации о версии/сборке из релизов.

const GITHUB_API: &str = "https://api.github.com/";

/// Проверить соединение с GitHub (блокирующий вызов — запускать в фоне).
/// Возвращает короткую строку статуса для интерфейса.
pub fn check_github_connection() -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("TraySession-Updater/0.7")
        .build()
        .map_err(|e| format!("HTTP-клиент: {e}"))?;
    let resp = client
        .get(GITHUB_API)
        .send()
        .map_err(|e| format!("Нет соединения с GitHub: {e}"))?;
    let status = resp.status();
    if status.is_success() {
        Ok(format!("Соединение с GitHub: OK ({status})"))
    } else {
        Err(format!("GitHub ответил: {status}"))
    }
}
