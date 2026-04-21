use rusqlite::{Connection, Result, params};
use chrono::{DateTime, Utc, SecondsFormat};

#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    Create,
    Modify,
    Move,
    Open,
    Append,
    Write
}

impl OperationType {
    pub fn as_str(&self) -> &str {
        match self {
            OperationType::Create => "Create",
            OperationType::Modify => "Modify",
            OperationType::Move => "Move",
            OperationType::Open => "Open",
            OperationType::Append => "Append",
            OperationType::Write => "Write",
        }
    }
}

pub struct Digest {
    pub filepath:  String,
    pub hash:      String,
}

pub struct Operation {
    pub timestamp: String, // DateTime<Utc>,        // yyyy-mm-ddThh:mm:ss.ffffff
    pub operation: OperationType,
    pub filepath:  String,
    pub src_path:  Option<String>,
    pub pid:       Option<i32>,
    pub ppid:      Option<i32>,
}

pub struct EventRepository {
    conn: Connection,
}

impl EventRepository {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS Digest (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL DEFAULT (DATETIME('now')),
                filepath  TEXT NOT NULL,
                hash      TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS Operation (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                digest_id INTEGER NOT NULL  REFERENCES Digest(id),
                timestamp TEXT NOT NULL,
                operation TEXT NOT NULL CHECK(operation IN ('Create', 'Modify', 'Move', 'Open', 'Append', 'Write')),
                filepath  TEXT NOT NULL,
                src_path  TEXT,
                pid       INTEGER,
                ppid      INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_path_time ON Digest (filepath, created_at);
            CREATE INDEX IF NOT EXISTS idx_hash_time ON Digest (hash, created_at);
            CREATE INDEX IF NOT EXISTS idx_time      ON Digest (created_at);
            CREATE INDEX IF NOT EXISTS idx_pid_time  ON Operation (pid, timestamp);
            CREATE INDEX IF NOT EXISTS idx_ppid_time ON Operation (ppid, timestamp);
        ")?;

        Ok(Self { conn })
    }

    pub fn insert_digest(&self, event: &Digest) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO Digest (filepath, hash)
             VALUES (?1, ?2)",
            params![
                event.filepath,
                event.hash,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_operation(&self, digest_id: &i64, event: &Operation) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO Operation (digest_id, timestamp, operation, filepath, src_path, pid, ppid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                digest_id,
                event.timestamp, // .to_rfc3339_opts(SecondsFormat::Micros, true),
                event.operation.as_str(),
                event.filepath,
                event.src_path,
                event.pid,
                event.ppid,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn latest_hash_by_path(&self, filepath: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT hash FROM Digest
            WHERE filepath = ?1
            ORDER BY created_at DESC
            LIMIT 1"
        )?;

        let mut rows = stmt.query(params![filepath])?;

        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None), // 該当するエントリがなかった場合
        }
    }

    pub fn get_entries(&self, date: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT created_at, filepath, hash,
            FROM Digest
            WHERE created_at LIKE ?1
            ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map(
            params![format!("{}%", date)],
            |row| {
                let timestamp: String       = row.get(0)?;
                let filepath:  String       = row.get(2)?;
                let hash:      String       = row.get(3)?;

                // 既存のログファイルのフォーマットに合わせて文字列化
                let entry = format!("{},{},{}", timestamp, filepath, hash);
                Ok(entry)
            }
        )?;

        rows.collect()
    }
}
