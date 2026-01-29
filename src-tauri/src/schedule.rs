use base64::{engine::general_purpose, Engine as _};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use scraper::{Html, Selector};
use std::collections::HashSet;

use crate::db::ScheduleEntryInput;

const LOGIN_URL: &str = "http://jwxt.ahut.edu.cn/jsxsd/xk/LoginToXk";
const SCHEDULE_URL: &str = "http://jwxt.ahut.edu.cn/jsxsd/xskb/xskb_list.do";

pub struct ScheduleFetchResult {
    pub term: String,
    pub terms: Vec<String>,
    pub entries: Vec<ScheduleEntryInput>,
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
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_terms(html: &str) -> (Vec<String>, Option<String>) {
    let document = Html::parse_document(html);
    let selector = Selector::parse("#xnxq01id option").unwrap();
    let mut terms = Vec::new();
    let mut selected = None;
    for option in document.select(&selector) {
        let value = option
            .value()
            .attr("value")
            .map(|v| v.trim().to_string())
            .unwrap_or_default();
        if value.is_empty() {
            continue;
        }
        if option.value().attr("selected").is_some() {
            selected = Some(value.clone());
        }
        terms.push(value);
    }
    (terms, selected)
}

fn parse_period_index(label: &str) -> Option<i32> {
    let mut num = String::new();
    for ch in label.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
        } else if !num.is_empty() {
            break;
        }
    }
    num.parse::<i32>().ok()
}

fn parse_week_numbers(week_text: &str) -> Vec<i32> {
    let mut filtered = week_text.to_string();
    let is_odd = filtered.contains("单周");
    let is_even = filtered.contains("双周");
    if let Some(idx) = filtered.find('周') {
        filtered = filtered[..idx].to_string();
    }
    let mut cleaned = String::new();
    for ch in filtered.chars() {
        if ch.is_ascii_digit() || ch == '-' || ch == ',' || ch == '，' {
            cleaned.push(if ch == '，' { ',' } else { ch });
        } else {
            cleaned.push(' ');
        }
    }
    let mut weeks = Vec::new();
    for part in cleaned.split_whitespace() {
        for token in part.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            if let Some((start, end)) = token.split_once('-') {
                if let (Ok(s), Ok(e)) = (start.parse::<i32>(), end.parse::<i32>()) {
                    let (min, max) = if s <= e { (s, e) } else { (e, s) };
                    for n in min..=max {
                        weeks.push(n);
                    }
                }
            } else if let Ok(n) = token.parse::<i32>() {
                weeks.push(n);
            }
        }
    }
    if is_odd {
        weeks.retain(|n| n % 2 == 1);
    } else if is_even {
        weeks.retain(|n| n % 2 == 0);
    }
    weeks.sort();
    weeks.dedup();
    weeks
}

fn split_blocks(lines: &[String]) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 5 {
            if !current.is_empty() {
                blocks.push(current);
                current = Vec::new();
            }
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        current.push(trimmed.to_string());
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn parse_cell_entries(
    term: &str,
    weekday: i32,
    period_label: &str,
    period_index: Option<i32>,
    cell: scraper::element_ref::ElementRef,
) -> Vec<ScheduleEntryInput> {
    let div_selector = Selector::parse("div.kbcontent1").unwrap();
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for div in cell.select(&div_selector) {
        let raw = div.text().collect::<Vec<_>>().join("\n");
        if raw.trim().is_empty() {
            continue;
        }
        let lines = raw
            .split('\n')
            .map(|line| normalize_text(line))
            .collect::<Vec<_>>();
        let blocks = split_blocks(&lines);
        for block in blocks {
            if block.is_empty() {
                continue;
            }
            let course_name = block.first().cloned().unwrap_or_default();
            if course_name.is_empty() {
                continue;
            }
            let mut week_text = None;
            let mut location = None;
            for line in block.iter().skip(1) {
                if line.contains('周') {
                    week_text = Some(line.clone());
                } else {
                    location = Some(line.clone());
                }
            }
            let week_numbers = week_text
                .as_deref()
                .map(parse_week_numbers)
                .unwrap_or_default();
            let key = format!(
                "{}|{}|{}|{}|{:?}",
                term, weekday, period_label, course_name, week_text
            );
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);
            entries.push(ScheduleEntryInput {
                term: term.to_string(),
                weekday,
                period_label: period_label.to_string(),
                period_index,
                course_name,
                teacher: None,
                location,
                week_text,
                week_numbers,
            });
        }
    }
    entries
}

fn parse_schedule_entries(html: &str, term: &str) -> Result<Vec<ScheduleEntryInput>, String> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("#kbtable tr")
        .map_err(|e| format!("Invalid selector: {}", e))?;
    let th_selector = Selector::parse("th")
        .map_err(|e| format!("Invalid selector: {}", e))?;
    let td_selector = Selector::parse("td")
        .map_err(|e| format!("Invalid selector: {}", e))?;

    let mut entries = Vec::new();
    let mut weekday_count = 0usize;
    for (row_index, row) in document.select(&row_selector).enumerate() {
        if row_index == 0 {
            let headers = row
                .select(&th_selector)
                .skip(1)
                .map(|cell| normalize_text(&cell.text().collect::<Vec<_>>().join("")))
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>();
            weekday_count = headers.len();
            continue;
        }

        let mut th_iter = row.select(&th_selector);
        let period_label = th_iter
            .next()
            .map(|cell| normalize_text(&cell.text().collect::<Vec<_>>().join("")))
            .unwrap_or_default();
        if period_label.contains("备注") {
            continue;
        }
        if period_label.is_empty() {
            continue;
        }
        let period_index = parse_period_index(&period_label);

        let cells = row.select(&td_selector).collect::<Vec<_>>();
        for (col_index, cell) in cells.iter().enumerate() {
            if weekday_count > 0 && col_index >= weekday_count {
                continue;
            }
            let weekday = (col_index + 1) as i32;
            let mut cell_entries =
                parse_cell_entries(term, weekday, &period_label, period_index, *cell);
            entries.append(&mut cell_entries);
        }
    }
    Ok(entries)
}

pub async fn fetch_schedule(
    username: &str,
    password: &str,
    term: Option<String>,
) -> Result<ScheduleFetchResult, String> {
    let client = build_client()?;
    login(username, password, &client).await?;

    let initial_html = client
        .get(SCHEDULE_URL)
        .send()
        .await
        .map_err(|e| format!("Schedule request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read schedule response: {}", e))?;

    let (terms, selected_term) = parse_terms(&initial_html);
    let selected_term = term.or(selected_term).unwrap_or_default();
    if selected_term.is_empty() {
        return Err("未获取到学期信息".to_string());
    }

    let html = if terms.is_empty() || selected_term.is_empty() {
        initial_html
    } else {
        let params = [("xnxq01id", selected_term.as_str()), ("zc", ""), ("sfFD", "1")];
        client
            .post(SCHEDULE_URL)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Schedule request failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Failed to read schedule response: {}", e))?
    };

    let entries = parse_schedule_entries(&html, &selected_term)?;
    Ok(ScheduleFetchResult {
        term: selected_term,
        terms,
        entries,
    })
}
