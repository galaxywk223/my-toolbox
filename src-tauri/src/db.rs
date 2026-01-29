use rusqlite::{params, Connection, OptionalExtension, Result};
pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(db_path: &str) -> Result<Self> {
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
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in rows {
            let col_name = col?;
            if col_name.eq_ignore_ascii_case("name") {
                has_name_column = true;
            } else if col_name.eq_ignore_ascii_case("class_name") {
                has_class_column = true;
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
