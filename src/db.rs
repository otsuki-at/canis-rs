use rusqlite::{Connection, Result, params};
use chrono::{DateTime, Utc, SecondsFormat};
use url::Url;

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
    pub uri:       Url,
    pub hash:      String,
}

pub struct Operation {
    pub timestamp: String, // DateTime<Utc>,        // yyyy-mm-ddThh:mm:ss.ffffff
    pub operation: OperationType,
    pub uri:       Url,
    pub src_path:  Option<Url>,
}

pub struct Process {
    pub starttime: String, // DateTime<Utc>,        // yyyy-mm-ddThh:mm:ss.ffffff
    pub pid:        i32,
    pub ppid:       i32,
    pub exe:        String,
    pub cmd:        String,
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
                uri       TEXT NOT NULL,
                hash      TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS Operation (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                digest_id INTEGER NOT NULL  REFERENCES Digest(id),
                timestamp TEXT NOT NULL,
                operation TEXT NOT NULL CHECK(operation IN ('Create', 'Modify', 'Move', 'Open', 'Append', 'Write')),
                uri       TEXT NOT NULL,
                src_path  TEXT
            );
            CREATE TABLE IF NOT EXISTS Process (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_id INTEGER NOT NULL  REFERENCES Operation(id),
                starttime TEXT NOT NULL,
                pid       INTEGER NOT NULL,
                ppid      INTEGER NOT NULL,
                exe       TEXT NOT NULL,
                cmd       TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_path_time ON Digest (uri, created_at);
            CREATE INDEX IF NOT EXISTS idx_hash_time ON Digest (hash, created_at);
            CREATE INDEX IF NOT EXISTS idx_time      ON Digest (created_at);
            CREATE INDEX IF NOT EXISTS idx_pid_time  ON Process (pid, starttime);
            CREATE INDEX IF NOT EXISTS idx_ppid_time ON Process (ppid, starttime);
        ")?;

        Ok(Self { conn })
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    pub fn insert_digest(&self, event: &Digest) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO Digest (uri, hash)
             VALUES (?1, ?2)",
            params![
                event.uri.as_str(),
                event.hash,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_operation(&self, digest_id: &i64, event: &Operation) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO Operation (digest_id, timestamp, operation, uri, src_path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                digest_id,
                event.timestamp, // .to_rfc3339_opts(SecondsFormat::Micros, true),
                event.operation.as_str(),
                event.uri.as_str(),
                event.src_path.as_ref().map(|u| u.as_str()),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_process(&self, operation_id: &i64, event: &Process) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO Process (operation_id, starttime, pid, ppid, exe, cmd)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                operation_id,
                event.starttime, // .to_rfc3339_opts(SecondsFormat::Micros, true),
                event.pid,
                event.ppid,
                event.exe,
                event.cmd,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn latest_hash_by_path(&self, uri: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT hash FROM Digest
            WHERE uri = ?1
            ORDER BY created_at DESC
            LIMIT 1"
        )?;

        let mut rows = stmt.query(params![uri])?;

        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None), // 該当するエントリがなかった場合
        }
    }

    pub fn get_entries(&self, date: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT created_at, uri, hash
            FROM Digest
            WHERE created_at LIKE ?1
            ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map(
            params![format!("{}%", date)],
            |row| {
                let timestamp: String       = row.get(0)?;
                let uri:       String       = row.get(1)?;
                let hash:      String       = row.get(2)?;

                // 既存のログファイルのフォーマットに合わせて文字列化
                let entry = format!("{},{},{}", timestamp, uri, hash);
                Ok(entry)
            }
        )?;

        rows.collect()
    }
}
