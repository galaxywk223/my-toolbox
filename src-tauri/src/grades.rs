use base64::{engine::general_purpose, Engine as _};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use scraper::{Html, Selector};
use std::collections::HashSet;

use crate::db::{GradeRecordInput, PlanCourseInput};

const LOGIN_URL: &str = "http://jwxt.ahut.edu.cn/jsxsd/xk/LoginToXk";
const GRADES_URL: &str = "http://jwxt.ahut.edu.cn/jsxsd/kscj/cjcx_list";
const MINOR_URL: &str = "http://jwxt.ahut.edu.cn/jsxsd/pyfa/fxpyfa_query";
const MAJOR_URL: &str = "http://jwxt.ahut.edu.cn/jsxsd/pyfa/pyfa_query";

pub struct GradeFetchResult {
    pub grades: Vec<GradeRecordInput>,
    pub minor_codes: Vec<String>,
    pub minor_names: Vec<String>,
    pub major_plan: Vec<PlanCourseInput>,
    pub minor_plan: Vec<PlanCourseInput>,
}

fn build_client() -> Result<reqwest::Client, String> {
    let headers = [
        ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36"),
        ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
        ("Accept-Language", "zh-CN,zh-TW;q=0.9,zh-HK;q=0.8,zh;q=0.7"),
    ];

    let mut default_headers = HeaderMap::new();
    for (key, value) in &headers {
        let header_name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| format!("Invalid header name {}: {}", key, e))?;
        let header_value =
            HeaderValue::from_str(value).map_err(|e| format!("Invalid header value: {}", e))?;
        default_headers.insert(header_name, header_value);
    }

    reqwest::Client::builder()
        .default_headers(default_headers)
        .cookie_store(true)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn encode_login_params(username: &str, password: &str) -> String {
    let username_encoded = general_purpose::STANDARD.encode(username.as_bytes());
    let password_encoded = general_purpose::STANDARD.encode(password.as_bytes());
    format!("{}%%%{}", username_encoded, password_encoded)
}

async fn login(username: &str, password: &str, client: &reqwest::Client) -> Result<(), String> {
    let encoded_value = encode_login_params(username, password);
    let params = [
        ("userAccount", username),
        ("userPassword", ""),
        ("encoded", encoded_value.as_str()),
        ("pwdstr1", ""),
        ("pwdstr2", ""),
    ];

    let response = client
        .post(LOGIN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Login request failed: {}", e))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read login response: {}", e))?;

    if text.contains("用户名或密码错误") {
        return Err("用户名或密码错误".to_string());
    }
    if text.contains("验证码") {
        return Err("登录需要验证码".to_string());
    }
    Ok(())
}

fn normalize_text(value: &str) -> String {
    value
        .replace('\u{a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_number(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<f32>().ok()
}

fn parse_grades(
    html: &str,
    minor_codes: &HashSet<String>,
    minor_names: &HashSet<String>,
) -> Result<Vec<GradeRecordInput>, String> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("#dataList tr")
        .map_err(|e| format!("Invalid selector: {}", e))?;
    let cell_selector = Selector::parse("td")
        .map_err(|e| format!("Invalid selector: {}", e))?;

    let mut results = Vec::new();
    for row in document.select(&row_selector) {
        let cells: Vec<String> = row
            .select(&cell_selector)
            .map(|cell| normalize_text(&cell.text().collect::<Vec<_>>().join("")))
            .collect();
        if cells.is_empty() {
            continue;
        }
        if cells.len() < 10 {
            continue;
        }

        let term = cells.get(1).cloned().unwrap_or_default();
        let course_code = cells.get(2).cloned().unwrap_or_default();
        let course_name = cells.get(3).cloned().unwrap_or_default();
        if term.is_empty() || course_code.is_empty() || course_name.is_empty() {
            continue;
        }

        let group_name = cells.get(4).cloned().unwrap_or_default();
        let score = cells.get(5).cloned().filter(|s| !s.is_empty());
        let score_flag = cells.get(6).cloned().filter(|s| !s.is_empty());
        let credit = cells.get(7).and_then(|v| parse_number(v));
        let total_hours = cells.get(8).and_then(|v| parse_number(v));
        let gpa = cells.get(9).and_then(|v| parse_number(v));
        let makeup_term = cells.get(10).cloned().filter(|s| !s.is_empty());
        let exam_mode = cells.get(11).cloned().filter(|s| !s.is_empty());
        let exam_type = cells.get(12).cloned().filter(|s| !s.is_empty());
        let course_attr = cells.get(13).cloned().filter(|s| !s.is_empty());
        let course_nature = cells.get(14).cloned().filter(|s| !s.is_empty());
        let general_type = cells.get(15).cloned().filter(|s| !s.is_empty());
        let is_minor =
            minor_codes.contains(&course_code) || minor_names.contains(&course_name);

        results.push(GradeRecordInput {
            term,
            course_code,
            course_name,
            group_name,
            score,
            score_flag,
            credit,
            total_hours,
            gpa,
            makeup_term,
            exam_mode,
            exam_type,
            course_attr,
            course_nature,
            general_type,
            is_minor,
        });
    }
    Ok(results)
}

fn parse_plan_courses(html: &str, is_minor: bool) -> Result<Vec<PlanCourseInput>, String> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("#dataList tr")
        .map_err(|e| format!("Invalid selector: {}", e))?;
    let cell_selector = Selector::parse("td")
        .map_err(|e| format!("Invalid selector: {}", e))?;
    let mut results = Vec::new();
    for row in document.select(&row_selector) {
        let cells: Vec<String> = row
            .select(&cell_selector)
            .map(|cell| normalize_text(&cell.text().collect::<Vec<_>>().join("")))
            .collect();
        if cells.len() < 6 {
            continue;
        }
        let term = cells.get(1).cloned().unwrap_or_default();
        let course_code = cells.get(2).cloned().unwrap_or_default();
        let course_name = cells.get(3).cloned().unwrap_or_default();
        if term.is_empty() || course_code.is_empty() || course_name.is_empty() {
            continue;
        }
        let credit = cells.get(5).and_then(|v| parse_number(v));
        let total_hours = cells.get(6).and_then(|v| parse_number(v));
        let exam_mode = cells.get(7).cloned().filter(|s| !s.is_empty());
        let (course_nature, course_attr) = if is_minor {
            (
                None,
                cells.get(8).cloned().filter(|s| !s.is_empty()),
            )
        } else {
            (
                cells.get(8).cloned().filter(|s| !s.is_empty()),
                cells.get(9).cloned().filter(|s| !s.is_empty()),
            )
        };

        results.push(PlanCourseInput {
            term,
            course_code,
            course_name,
            credit,
            total_hours,
            exam_mode,
            course_nature,
            course_attr,
            is_minor,
        });
    }
    Ok(results)
}

async fn fetch_plan_courses(
    client: &reqwest::Client,
    url: &str,
    is_minor: bool,
) -> Result<Vec<PlanCourseInput>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Plan request failed: {}", e))?;
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read plan response: {}", e))?;
    parse_plan_courses(&text, is_minor)
}

pub async fn fetch_grades(
    username: &str,
    password: &str,
) -> Result<GradeFetchResult, String> {
    let client = build_client()?;
    login(username, password, &client).await?;

    let minor_plan = fetch_plan_courses(&client, MINOR_URL, true).await.unwrap_or_default();
    let major_plan = fetch_plan_courses(&client, MAJOR_URL, false).await.unwrap_or_default();
    let mut minor_codes_set = HashSet::new();
    let mut minor_names_set = HashSet::new();
    for item in &minor_plan {
        minor_codes_set.insert(item.course_code.clone());
        minor_names_set.insert(item.course_name.clone());
    }
    let mut minor_codes: Vec<String> = minor_codes_set.iter().cloned().collect();
    let mut minor_names: Vec<String> = minor_names_set.iter().cloned().collect();
    minor_codes.sort();
    minor_names.sort();
    let params = [("kksj", ""), ("kcxz", ""), ("kcmc", ""), ("xsfs", "all")];
    let response = client
        .post(GRADES_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Grade request failed: {}", e))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read grade response: {}", e))?;

    let grades = parse_grades(&text, &minor_codes_set, &minor_names_set)?;
    Ok(GradeFetchResult {
        grades,
        minor_codes,
        minor_names,
        major_plan,
        minor_plan,
    })
}
