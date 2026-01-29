use rusqlite::{params, Connection, OptionalExtension, Result};
use std::fs;
use std::path::{Path, PathBuf};
pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS password_results (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                name TEXT,
                class_name TEXT,
                password_date TEXT,
                encoded_value TEXT,
                year INTEGER,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                status TEXT DEFAULT 'success'
            )",
            [],
        )?;
        // Best-effort migration for existing DBs: add name/class columns if missing.
        let mut stmt = self
            .conn
            .prepare("PRAGMA table_info(password_results)")?;
        let mut has_name_column = false;
        let mut has_class_column = false;
        let mut has_show_column = false;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in rows {
            let col_name = col?;
            if col_name.eq_ignore_ascii_case("name") {
                has_name_column = true;
            } else if col_name.eq_ignore_ascii_case("class_name") {
                has_class_column = true;
            } else if col_name.eq_ignore_ascii_case("show_in_grades") {
                has_show_column = true;
            }
        }
        if !has_name_column {
            self.conn
                .execute("ALTER TABLE password_results ADD COLUMN name TEXT", [])?;
        }
        if !has_class_column {
            self.conn
                .execute("ALTER TABLE password_results ADD COLUMN class_name TEXT", [])?;
        }
        if !has_show_column {
            self.conn.execute(
                "ALTER TABLE password_results ADD COLUMN show_in_grades INTEGER DEFAULT 0",
                [],
            )?;
            self.conn.execute(
                "UPDATE password_results SET show_in_grades = 0 WHERE show_in_grades IS NULL",
                [],
            )?;
        }
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS plan_courses (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                term TEXT NOT NULL,
                course_code TEXT NOT NULL,
                course_name TEXT NOT NULL,
                credit REAL,
                total_hours REAL,
                exam_mode TEXT,
                course_nature TEXT,
                course_attr TEXT,
                is_minor INTEGER DEFAULT 0,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schedule_terms (
                term TEXT PRIMARY KEY,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schedule_entries (
                id INTEGER PRIMARY KEY,
                term TEXT NOT NULL,
                weekday INTEGER NOT NULL,
                period_label TEXT NOT NULL,
                period_index INTEGER,
                course_name TEXT NOT NULL,
                teacher TEXT,
                location TEXT,
                week_text TEXT,
                week_numbers TEXT,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_plan_courses_unique
             ON plan_courses(username, term, course_code, is_minor)",
            [],
        )?;
        // Merge legacy students table into password_results, then drop it.
        let students_table = self
            .conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='students' LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if students_table.is_some() {
            let _ = self.conn.execute(
                "INSERT INTO password_results (username, name, class_name)
                 SELECT username, name, class_name FROM students
                 ON CONFLICT(username) DO UPDATE SET
                   name = excluded.name,
                   class_name = excluded.class_name",
                [],
            );
            self.conn.execute("DROP TABLE IF EXISTS students", [])?;
        }
        // Remove duplicate usernames, keep the latest record by id.
        self.conn.execute(
            "DELETE FROM password_results
             WHERE id NOT IN (
               SELECT MAX(id) FROM password_results GROUP BY username
             )",
            [],
        )?;
        // Normalize schema to allow NULL date fields when importing student info.
        let mut needs_schema_upgrade = false;
        let mut stmt = self
            .conn
            .prepare("PRAGMA table_info(password_results)")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(3)?))
        })?;
        for row in rows {
            let (name, notnull) = row?;
            if (name.eq_ignore_ascii_case("password_date")
                || name.eq_ignore_ascii_case("encoded_value")
                || name.eq_ignore_ascii_case("year"))
                && notnull == 1
            {
                needs_schema_upgrade = true;
                break;
            }
        }
        if needs_schema_upgrade {
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS password_results_new (
                    id INTEGER PRIMARY KEY,
                    username TEXT NOT NULL,
                    name TEXT,
                    class_name TEXT,
                    password_date TEXT,
                    encoded_value TEXT,
                    year INTEGER,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    status TEXT DEFAULT 'success'
                )",
                [],
            )?;
            self.conn.execute(
                "INSERT INTO password_results_new
                 (id, username, name, class_name, password_date, encoded_value, year, created_at, status)
                 SELECT id, username, name, class_name, password_date, encoded_value, year, created_at, status
                 FROM password_results",
                [],
            )?;
            self.conn.execute("DROP TABLE password_results", [])?;
            self.conn.execute("ALTER TABLE password_results_new RENAME TO password_results", [])?;
        }
        // Enforce uniqueness by username going forward.
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_password_results_username
             ON password_results(username)",
            [],
        )?;

        let _ = self.conn.execute("DROP TABLE IF EXISTS grade_users", []);

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS grade_records (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                term TEXT NOT NULL,
                course_code TEXT NOT NULL,
                course_name TEXT NOT NULL,
                group_name TEXT NOT NULL DEFAULT '',
                score TEXT,
                score_flag TEXT,
                credit REAL,
                total_hours REAL,
                gpa REAL,
                makeup_term TEXT,
                exam_mode TEXT,
                exam_type TEXT,
                course_attr TEXT,
                course_nature TEXT,
                general_type TEXT,
                is_minor INTEGER DEFAULT 0,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_grade_records_unique
             ON grade_records(username, term, course_code, group_name)",
            [],
        )?;
        let mut stmt = self.conn.prepare("PRAGMA table_info(grade_records)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut has_minor_column = false;
        for col in rows {
            if col?.eq_ignore_ascii_case("is_minor") {
                has_minor_column = true;
                break;
            }
        }
        if !has_minor_column {
            self.conn.execute(
                "ALTER TABLE grade_records ADD COLUMN is_minor INTEGER DEFAULT 0",
                [],
            )?;
        }
        Ok(())
    }

    pub fn insert_result(
        &self,
        username: &str,
        name: &str,
        class_name: &str,
        password_date: &str,
        encoded_value: &str,
        year: i32,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO password_results (username, name, class_name, password_date, encoded_value, year)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(username) DO UPDATE SET
               name = excluded.name,
               class_name = excluded.class_name,
               password_date = excluded.password_date,
               encoded_value = excluded.encoded_value,
               year = excluded.year",
            [
                username,
                name,
                class_name,
                password_date,
                encoded_value,
                &year.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn get_result_by_username(&self, username: &str) -> Result<Option<PasswordResult>> {
        self.conn
            .query_row(
                "SELECT id, username, name, class_name, password_date, encoded_value, year, created_at, status
                 FROM password_results WHERE username = ?1 LIMIT 1",
                [username],
                |row| {
                    Ok(PasswordResult {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        name: row.get(2)?,
                        class_name: row.get(3)?,
                        password_date: row.get(4)?,
                        encoded_value: row.get(5)?,
                        year: row.get(6)?,
                        created_at: row.get(7)?,
                        status: row.get(8)?,
                    })
                },
            )
            .optional()
    }

    pub fn get_all_results(&self) -> Result<Vec<PasswordResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, name, class_name, password_date, encoded_value, year, created_at, status 
             FROM password_results ORDER BY created_at DESC"
        )?;

        let results = stmt.query_map([], |row| {
            Ok(PasswordResult {
                id: row.get(0)?,
                username: row.get(1)?,
                name: row.get(2)?,
                class_name: row.get(3)?,
                password_date: row.get(4)?,
                encoded_value: row.get(5)?,
                year: row.get(6)?,
                created_at: row.get(7)?,
                status: row.get(8)?,
            })
        })?;

        let mut all_results = Vec::new();
        for result in results {
            all_results.push(result?);
        }
        Ok(all_results)
    }

    pub fn upsert_students(&mut self, students: &[StudentImport]) -> Result<(usize, usize)> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0usize;
        let mut updated = 0usize;
        for student in students {
            let changes = tx.execute(
                "INSERT INTO password_results (username, name, class_name)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(username) DO UPDATE SET
                   name = excluded.name,
                   class_name = excluded.class_name",
                [&student.username, &student.name, &student.class_name],
            )?;
            if changes == 1 {
                inserted += 1;
            } else {
                updated += 1;
            }
        }
        tx.commit()?;
        Ok((inserted, updated))
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PasswordResult {
    pub id: i32,
    pub username: String,
    pub name: Option<String>,
    pub class_name: Option<String>,
    pub password_date: Option<String>,
    pub encoded_value: Option<String>,
    pub year: Option<i32>,
    pub created_at: String,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct StudentImport {
    pub username: String,
    pub name: String,
    pub class_name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct DateImport {
    pub username: String,
    pub password_date: String,
    pub encoded_value: Option<String>,
}

impl Database {
    pub fn upsert_dates(&mut self, dates: &[DateImport]) -> Result<(usize, usize)> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0usize;
        let mut updated = 0usize;
        for item in dates {
            let year_value = item
                .password_date
                .get(0..4)
                .and_then(|y| y.parse::<i32>().ok());
            let encoded_value = item
                .encoded_value
                .as_deref()
                .filter(|value| !value.trim().is_empty());
            let changes = tx.execute(
                "INSERT INTO password_results (username, password_date, encoded_value, year)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(username) DO UPDATE SET
                   password_date = excluded.password_date,
                   encoded_value = excluded.encoded_value,
                   year = excluded.year",
                params![&item.username, &item.password_date, encoded_value, year_value],
            )?;
            if changes == 1 {
                inserted += 1;
            } else {
                updated += 1;
            }
        }
        tx.commit()?;
        Ok((inserted, updated))
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GradeUser {
    pub username: String,
    pub display_name: Option<String>,
    pub class_name: Option<String>,
    pub created_at: String,
    pub last_updated: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GradeRecord {
    pub id: i32,
    pub username: String,
    pub term: String,
    pub course_code: String,
    pub course_name: String,
    pub group_name: String,
    pub score: Option<String>,
    pub score_flag: Option<String>,
    pub credit: Option<f32>,
    pub total_hours: Option<f32>,
    pub gpa: Option<f32>,
    pub makeup_term: Option<String>,
    pub exam_mode: Option<String>,
    pub exam_type: Option<String>,
    pub course_attr: Option<String>,
    pub course_nature: Option<String>,
    pub general_type: Option<String>,
    pub is_minor: bool,
    pub updated_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GradeRecordInput {
    pub term: String,
    pub course_code: String,
    pub course_name: String,
    pub group_name: String,
    pub score: Option<String>,
    pub score_flag: Option<String>,
    pub credit: Option<f32>,
    pub total_hours: Option<f32>,
    pub gpa: Option<f32>,
    pub makeup_term: Option<String>,
    pub exam_mode: Option<String>,
    pub exam_type: Option<String>,
    pub course_attr: Option<String>,
    pub course_nature: Option<String>,
    pub general_type: Option<String>,
    pub is_minor: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PlanCourse {
    pub id: i32,
    pub term: String,
    pub course_code: String,
    pub course_name: String,
    pub credit: Option<f32>,
    pub total_hours: Option<f32>,
    pub exam_mode: Option<String>,
    pub course_nature: Option<String>,
    pub course_attr: Option<String>,
    pub is_minor: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ScheduleTerm {
    pub term: String,
    pub updated_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ScheduleEntry {
    pub id: i32,
    pub term: String,
    pub weekday: i32,
    pub period_label: String,
    pub period_index: Option<i32>,
    pub course_name: String,
    pub teacher: Option<String>,
    pub location: Option<String>,
    pub week_text: Option<String>,
    pub week_numbers: Vec<i32>,
    pub updated_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ScheduleEntryInput {
    pub term: String,
    pub weekday: i32,
    pub period_label: String,
    pub period_index: Option<i32>,
    pub course_name: String,
    pub teacher: Option<String>,
    pub location: Option<String>,
    pub week_text: Option<String>,
    pub week_numbers: Vec<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct UpdateScheduleEntryInput {
    pub id: i32,
    pub course_name: Option<String>,
    pub teacher: Option<String>,
    pub location: Option<String>,
    pub week_text: Option<String>,
    pub week_numbers: Option<Vec<i32>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PlanCourseInput {
    pub term: String,
    pub course_code: String,
    pub course_name: String,
    pub credit: Option<f32>,
    pub total_hours: Option<f32>,
    pub exam_mode: Option<String>,
    pub course_nature: Option<String>,
    pub course_attr: Option<String>,
    pub is_minor: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct UpdateGradeRecordInput {
    pub id: i32,
    pub score: Option<String>,
    pub score_flag: Option<String>,
    pub credit: Option<f32>,
    pub total_hours: Option<f32>,
    pub gpa: Option<f32>,
    pub makeup_term: Option<String>,
    pub exam_type: Option<String>,
    pub course_attr: Option<String>,
    pub course_nature: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct UpdatePlanCourseInput {
    pub id: i32,
    pub course_name: Option<String>,
    pub credit: Option<f32>,
    pub total_hours: Option<f32>,
    pub exam_mode: Option<String>,
    pub course_nature: Option<String>,
    pub course_attr: Option<String>,
}

impl Database {
    pub fn get_grade_users(&self) -> Result<Vec<GradeUser>> {
        let mut stmt = self.conn.prepare(
            "SELECT pr.username,
                    pr.name,
                    pr.class_name,
                    pr.created_at,
                    MAX(gr.updated_at) as last_updated
             FROM password_results pr
             LEFT JOIN grade_records gr ON gr.username = pr.username
             WHERE pr.password_date IS NOT NULL
               AND TRIM(pr.password_date) <> ''
               AND COALESCE(pr.show_in_grades, 0) = 1
             GROUP BY pr.username, pr.name, pr.class_name, pr.created_at
             ORDER BY last_updated DESC, pr.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(GradeUser {
                username: row.get(0)?,
                display_name: row.get(1)?,
                class_name: row.get(2)?,
                created_at: row.get(3)?,
                last_updated: row.get(4)?,
            })
        })?;
        let mut users = Vec::new();
        for row in rows {
            users.push(row?);
        }
        Ok(users)
    }

    pub fn ensure_user_in_password_results(&mut self, username: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO password_results (username, show_in_grades)
             VALUES (?1, 1)
             ON CONFLICT(username) DO UPDATE SET
               show_in_grades = 1",
            params![username],
        )?;
        Ok(())
    }

    pub fn save_user_password(&mut self, username: &str, password: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE password_results SET password_date = ?1 WHERE username = ?2",
            params![password, username],
        )?;
        Ok(())
    }

    pub fn get_saved_password(&self, username: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT password_date FROM password_results WHERE username = ?1 LIMIT 1",
                params![username],
                |row| row.get::<_, String>(0),
            )
            .optional()
    }

    pub fn hide_grade_user(&mut self, username: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE password_results SET show_in_grades = 0 WHERE username = ?1",
            params![username],
        )?;
        tx.execute(
            "DELETE FROM grade_records WHERE username = ?1",
            params![username],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_password_result(
        &mut self,
        username: &str,
        name: Option<&str>,
        class_name: Option<&str>,
        password_date: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE password_results
             SET name = COALESCE(?1, name),
                 class_name = COALESCE(?2, class_name),
                 password_date = COALESCE(?3, password_date)
             WHERE username = ?4",
            params![name, class_name, password_date, username],
        )?;
        Ok(())
    }

    pub fn delete_password_result(&mut self, username: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM password_results WHERE username = ?1",
            params![username],
        )?;
        Ok(())
    }

    pub fn count_user_relations(&self, username: &str) -> Result<(i64, i64)> {
        let grade_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM grade_records WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )?;
        let plan_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM plan_courses WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )?;
        Ok((grade_count, plan_count))
    }

    pub fn get_grades_by_username(&self, username: &str) -> Result<Vec<GradeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, term, course_code, course_name, group_name, score, score_flag,
                    credit, total_hours, gpa, makeup_term, exam_mode, exam_type, course_attr,
                    course_nature, general_type, is_minor, updated_at
             FROM grade_records
             WHERE username = ?1
             ORDER BY term DESC, course_code ASC",
        )?;
        let rows = stmt.query_map([username], |row| {
            Ok(GradeRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                term: row.get(2)?,
                course_code: row.get(3)?,
                course_name: row.get(4)?,
                group_name: row.get(5)?,
                score: row.get(6)?,
                score_flag: row.get(7)?,
                credit: row.get(8)?,
                total_hours: row.get(9)?,
                gpa: row.get(10)?,
                makeup_term: row.get(11)?,
                exam_mode: row.get(12)?,
                exam_type: row.get(13)?,
                course_attr: row.get(14)?,
                course_nature: row.get(15)?,
                general_type: row.get(16)?,
                is_minor: row.get(17)?,
                updated_at: row.get(18)?,
            })
        })?;
        let mut grades = Vec::new();
        for row in rows {
            grades.push(row?);
        }
        Ok(grades)
    }

    pub fn get_all_grades(&self) -> Result<Vec<GradeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, term, course_code, course_name, group_name, score, score_flag,
                    credit, total_hours, gpa, makeup_term, exam_mode, exam_type, course_attr,
                    course_nature, general_type, is_minor, updated_at
             FROM grade_records
             ORDER BY username ASC, term DESC, course_code ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(GradeRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                term: row.get(2)?,
                course_code: row.get(3)?,
                course_name: row.get(4)?,
                group_name: row.get(5)?,
                score: row.get(6)?,
                score_flag: row.get(7)?,
                credit: row.get(8)?,
                total_hours: row.get(9)?,
                gpa: row.get(10)?,
                makeup_term: row.get(11)?,
                exam_mode: row.get(12)?,
                exam_type: row.get(13)?,
                course_attr: row.get(14)?,
                course_nature: row.get(15)?,
                general_type: row.get(16)?,
                is_minor: row.get(17)?,
                updated_at: row.get(18)?,
            })
        })?;
        let mut grades = Vec::new();
        for row in rows {
            grades.push(row?);
        }
        Ok(grades)
    }

    pub fn upsert_grades(
        &mut self,
        username: &str,
        grades: &[GradeRecordInput],
    ) -> Result<(usize, usize)> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0usize;
        let mut updated = 0usize;
        for grade in grades {
            let changes = tx.execute(
                "INSERT INTO grade_records (
                    username, term, course_code, course_name, group_name, score, score_flag,
                    credit, total_hours, gpa, makeup_term, exam_mode, exam_type, course_attr,
                    course_nature, general_type, is_minor, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, CURRENT_TIMESTAMP)
                 ON CONFLICT(username, term, course_code, group_name) DO UPDATE SET
                    course_name = excluded.course_name,
                    score = excluded.score,
                    score_flag = excluded.score_flag,
                    credit = excluded.credit,
                    total_hours = excluded.total_hours,
                    gpa = excluded.gpa,
                    makeup_term = excluded.makeup_term,
                    exam_mode = excluded.exam_mode,
                    exam_type = excluded.exam_type,
                    course_attr = excluded.course_attr,
                    course_nature = excluded.course_nature,
                    general_type = excluded.general_type,
                    is_minor = excluded.is_minor,
                    updated_at = CURRENT_TIMESTAMP",
                params![
                    username,
                    grade.term,
                    grade.course_code,
                    grade.course_name,
                    grade.group_name,
                    grade.score,
                    grade.score_flag,
                    grade.credit,
                    grade.total_hours,
                    grade.gpa,
                    grade.makeup_term,
                    grade.exam_mode,
                    grade.exam_type,
                    grade.course_attr,
                    grade.course_nature,
                    grade.general_type,
                    grade.is_minor,
                ],
            )?;
            if changes == 1 {
                inserted += 1;
            } else {
                updated += 1;
            }
        }
        tx.commit()?;
        Ok((inserted, updated))
    }

    pub fn update_minor_flags(
        &mut self,
        username: &str,
        minor_codes: &[String],
        minor_names: &[String],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE grade_records SET is_minor = 0 WHERE username = ?1",
            params![username],
        )?;
        for code in minor_codes {
            let trimmed = code.trim();
            if trimmed.is_empty() {
                continue;
            }
            tx.execute(
                "UPDATE grade_records SET is_minor = 1 WHERE username = ?1 AND course_code = ?2",
                params![username, trimmed],
            )?;
        }
        for name in minor_names {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                continue;
            }
            tx.execute(
                "UPDATE grade_records SET is_minor = 1 WHERE username = ?1 AND course_name = ?2",
                params![username, trimmed],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_grade_record(&mut self, record: &UpdateGradeRecordInput) -> Result<()> {
        self.conn.execute(
            "UPDATE grade_records
             SET score = COALESCE(?1, score),
                 score_flag = COALESCE(?2, score_flag),
                 credit = COALESCE(?3, credit),
                 total_hours = COALESCE(?4, total_hours),
                 gpa = COALESCE(?5, gpa),
                 makeup_term = COALESCE(?6, makeup_term),
                 exam_type = COALESCE(?7, exam_type),
                 course_attr = COALESCE(?8, course_attr),
                 course_nature = COALESCE(?9, course_nature),
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?10",
            params![
                record.score,
                record.score_flag,
                record.credit,
                record.total_hours,
                record.gpa,
                record.makeup_term,
                record.exam_type,
                record.course_attr,
                record.course_nature,
                record.id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_grade_record(&mut self, id: i32) -> Result<()> {
        self.conn
            .execute("DELETE FROM grade_records WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn replace_plan_courses(
        &mut self,
        username: &str,
        is_minor: bool,
        courses: &[PlanCourseInput],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM plan_courses WHERE username = ?1 AND is_minor = ?2",
            params![username, is_minor],
        )?;
        for course in courses {
            if course.term.trim().is_empty()
                || course.course_code.trim().is_empty()
                || course.course_name.trim().is_empty()
            {
                continue;
            }
            tx.execute(
                "INSERT INTO plan_courses (
                    username, term, course_code, course_name, credit, total_hours,
                    exam_mode, course_nature, course_attr, is_minor, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP)",
                params![
                    username,
                    course.term,
                    course.course_code,
                    course.course_name,
                    course.credit,
                    course.total_hours,
                    course.exam_mode,
                    course.course_nature,
                    course.course_attr,
                    course.is_minor,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_pending_courses(
        &self,
        username: &str,
        category_flag: i32,
    ) -> Result<Vec<PlanCourse>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, term, course_code, course_name, credit, total_hours, exam_mode, course_nature,
                    course_attr, is_minor
             FROM plan_courses pc
             WHERE pc.username = ?1
               AND (?2 = -1 OR pc.is_minor = ?2)
               AND NOT EXISTS (
                   SELECT 1 FROM grade_records gr
                   WHERE gr.username = pc.username
                     AND (gr.course_code = pc.course_code OR gr.course_name = pc.course_name)
               )
             ORDER BY pc.term DESC, pc.course_code ASC",
        )?;
        let rows = stmt.query_map(params![username, category_flag], |row| {
            Ok(PlanCourse {
                id: row.get(0)?,
                term: row.get(1)?,
                course_code: row.get(2)?,
                course_name: row.get(3)?,
                credit: row.get(4)?,
                total_hours: row.get(5)?,
                exam_mode: row.get(6)?,
                course_nature: row.get(7)?,
                course_attr: row.get(8)?,
                is_minor: row.get::<_, i32>(9)? == 1,
            })
        })?;
        let mut pending = Vec::new();
        for row in rows {
            pending.push(row?);
        }
        Ok(pending)
    }

    pub fn update_plan_course(&mut self, course: &UpdatePlanCourseInput) -> Result<()> {
        self.conn.execute(
            "UPDATE plan_courses
             SET course_name = COALESCE(?1, course_name),
                 credit = COALESCE(?2, credit),
                 total_hours = COALESCE(?3, total_hours),
                 exam_mode = COALESCE(?4, exam_mode),
                 course_nature = COALESCE(?5, course_nature),
                 course_attr = COALESCE(?6, course_attr),
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?7",
            params![
                course.course_name,
                course.credit,
                course.total_hours,
                course.exam_mode,
                course.course_nature,
                course.course_attr,
                course.id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_plan_course(&mut self, id: i32) -> Result<()> {
        self.conn
            .execute("DELETE FROM plan_courses WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn upsert_schedule_terms(&mut self, terms: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for term in terms {
            let trimmed = term.trim();
            if trimmed.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT INTO schedule_terms (term)
                 VALUES (?1)
                 ON CONFLICT(term) DO UPDATE SET
                   updated_at = CURRENT_TIMESTAMP",
                params![trimmed],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_schedule_entries(
        &mut self,
        term: &str,
        entries: &[ScheduleEntryInput],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM schedule_entries WHERE term = ?1",
            params![term],
        )?;
        for entry in entries {
            let week_numbers = entry
                .week_numbers
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",");
            tx.execute(
                "INSERT INTO schedule_entries
                 (term, weekday, period_label, period_index, course_name, teacher, location, week_text, week_numbers)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    term,
                    entry.weekday,
                    entry.period_label,
                    entry.period_index,
                    entry.course_name,
                    entry.teacher,
                    entry.location,
                    entry.week_text,
                    week_numbers,
                ],
            )?;
        }
        tx.execute(
            "INSERT INTO schedule_terms (term)
             VALUES (?1)
             ON CONFLICT(term) DO UPDATE SET
               updated_at = CURRENT_TIMESTAMP",
            params![term],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_schedule_terms(&self) -> Result<Vec<ScheduleTerm>> {
        let mut stmt = self.conn.prepare(
            "SELECT term, updated_at FROM schedule_terms ORDER BY updated_at DESC, term DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ScheduleTerm {
                term: row.get(0)?,
                updated_at: row.get(1)?,
            })
        })?;
        let mut terms = Vec::new();
        for row in rows {
            terms.push(row?);
        }
        Ok(terms)
    }

    pub fn get_schedule_entries(&self, term: &str) -> Result<Vec<ScheduleEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, term, weekday, period_label, period_index, course_name, teacher, location,
                    week_text, week_numbers, updated_at
             FROM schedule_entries
             WHERE term = ?1
             ORDER BY weekday ASC, period_index ASC, course_name ASC",
        )?;
        let rows = stmt.query_map(params![term], |row| {
            let week_numbers: Option<String> = row.get(9)?;
            let weeks = week_numbers
                .unwrap_or_default()
                .split(',')
                .filter_map(|v| v.trim().parse::<i32>().ok())
                .collect::<Vec<_>>();
            Ok(ScheduleEntry {
                id: row.get(0)?,
                term: row.get(1)?,
                weekday: row.get(2)?,
                period_label: row.get(3)?,
                period_index: row.get(4)?,
                course_name: row.get(5)?,
                teacher: row.get(6)?,
                location: row.get(7)?,
                week_text: row.get(8)?,
                week_numbers: weeks,
                updated_at: row.get(10)?,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn update_schedule_entry(&mut self, entry: &UpdateScheduleEntryInput) -> Result<()> {
        let week_numbers = entry.week_numbers.as_ref().map(|weeks| {
            weeks
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",")
        });
        self.conn.execute(
            "UPDATE schedule_entries
             SET course_name = COALESCE(?1, course_name),
                 teacher = COALESCE(?2, teacher),
                 location = COALESCE(?3, location),
                 week_text = COALESCE(?4, week_text),
                 week_numbers = COALESCE(?5, week_numbers),
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?6",
            params![
                entry.course_name,
                entry.teacher,
                entry.location,
                entry.week_text,
                week_numbers,
                entry.id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_schedule_entry(&mut self, id: i32) -> Result<()> {
        self.conn
            .execute("DELETE FROM schedule_entries WHERE id = ?1", params![id])?;
        Ok(())
    }
}

pub fn resolve_db_path() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        std::env::current_dir()
            .map(|dir| dir.join("toolbox.db"))
            .map_err(|e| format!("Failed to resolve current dir: {}", e))
    } else {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Failed to resolve exe path: {}", e))?;
        let dir = exe
            .parent()
            .ok_or_else(|| "Failed to resolve exe directory".to_string())?;
        Ok(dir.join("toolbox.db"))
    }
}

pub fn resolve_old_db_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(dir) = std::env::current_dir() {
        paths.push(dir.join("password_results.db"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("password_results.db");
            if !paths.iter().any(|p| p == &candidate) {
                paths.push(candidate);
            }
        }
    }
    paths
}

pub fn migrate_if_needed() -> Result<(), String> {
    let db_path = resolve_db_path()?;
    if db_path.exists() {
        return Ok(());
    }
    for old_path in resolve_old_db_paths() {
        if old_path.exists() {
            fs::copy(&old_path, &db_path)
                .map_err(|e| format!("Failed to migrate database: {}", e))?;
            return Ok(());
        }
    }
    Ok(())
}
