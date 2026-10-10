// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Account-owned versioned settings documents. Domain validation belongs to
//! the service. Compare-and-swap prevents stale sessions or separate processes
//! from replacing newer state. Successful writes have committed with SQLite
//! synchronous=EXTRA; callers must complete them before acknowledging a save.
use crate::{AccountId, Error};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub const MAX_BYTES: usize = 512 * 1024;
const APPLICATION_ID: i64 = 0x4e465350;

#[derive(Clone, PartialEq, Eq)]
pub struct Document {
    pub revision: u64,
    pub bytes: Vec<u8>,
}
impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsDocument")
            .field("revision", &self.revision)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct Repository {
    root: PathBuf,
}
impl Repository {
    pub fn open_owned_directory(root: &Path) -> Result<Self, Error> {
        std::fs::create_dir_all(root).map_err(|_| Error::Storage)?;
        let meta = std::fs::symlink_metadata(root).map_err(|_| Error::Storage)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(Error::Config);
        }
        Ok(Self {
            root: root.canonicalize().map_err(|_| Error::Storage)?,
        })
    }
    pub fn path(&self, account: AccountId) -> PathBuf {
        self.root.join(format!("settings-{}.sqlite", account.hex()))
    }
    fn connect(&self, account: AccountId, create: bool) -> Result<Connection, Error> {
        let path = self.path(account);
        match std::fs::symlink_metadata(&path) {
            Ok(m) if !m.is_file() || m.file_type().is_symlink() || m.len() > 4 * 1024 * 1024 => {
                return Err(Error::Config);
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::Absent),
            Err(_) => return Err(Error::Storage),
        }
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let conn = Connection::open_with_flags(path, flags)?;
        conn.busy_timeout(Duration::from_millis(250))?;
        conn.set_limit(
            rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
            (MAX_BYTES + 1024) as i32,
        )?;
        conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH, 8192)?;
        conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 4)?;
        conn.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA synchronous=EXTRA; PRAGMA max_page_count=1024; PRAGMA journal_size_limit=1048576;")?;
        let synchronous: i64 = conn.query_row("PRAGMA synchronous", [], |r| r.get(0))?;
        if synchronous != 3 {
            return Err(Error::Storage);
        }
        Ok(conn)
    }
    fn check(conn: &Connection) -> Result<(), Error> {
        let app: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let objects: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )?;
        if app != APPLICATION_ID || version != 1 || objects != 1 {
            return Err(Error::Version);
        }
        Ok(())
    }
    fn document(conn: &Connection, account: AccountId) -> Result<Option<Document>, Error> {
        let rows: i64 = conn.query_row("SELECT count(*) FROM settings", [], |r| r.get(0))?;
        if rows > 1 {
            return Err(Error::Config);
        }
        let row = conn
            .query_row(
                "SELECT account, revision, value FROM settings WHERE singleton=1",
                [],
                |r| {
                    Ok((
                        r.get::<_, Vec<u8>>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(owner, revision, bytes)| {
            if owner != account.bytes() {
                return Err(Error::Identity);
            }
            if revision < 1 || bytes.is_empty() || bytes.len() > MAX_BYTES {
                return Err(Error::Config);
            }
            Ok(Document {
                revision: revision as u64,
                bytes,
            })
        })
        .transpose()
    }
    pub fn read(&self, account: AccountId) -> Result<Option<Document>, Error> {
        let mut conn = match self.connect(account, false) {
            Err(Error::Absent) => return Ok(None),
            other => other?,
        };
        let tx = conn.transaction()?;
        Self::check(&tx)?;
        Self::document(&tx, account)
    }
    /// `None` initializes an absent document. A stale revision returns false
    /// without changing the value. Retries must reload and reapply their delta.
    pub fn compare_exchange(
        &self,
        account: AccountId,
        expected: Option<u64>,
        bytes: &[u8],
    ) -> Result<bool, Error> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err(Error::Bounds);
        }
        let mut conn = self.connect(account, true)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let app: i64 = tx.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if app == 0 && version == 0 {
            let objects: i64 = tx.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            if objects != 0 {
                return Err(Error::Version);
            }
            tx.execute_batch(&format!("CREATE TABLE settings(singleton INTEGER PRIMARY KEY CHECK(singleton=1), account BLOB NOT NULL CHECK(length(account)=16), revision INTEGER NOT NULL CHECK(revision>0), value BLOB NOT NULL CHECK(length(value)>0 AND length(value)<={MAX_BYTES})) STRICT; PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version=1;"))?;
        }
        Self::check(&tx)?;
        let current = Self::document(&tx, account)?;
        if current.as_ref().map(|d| d.revision) != expected {
            return Ok(false);
        }
        let revision = expected
            .unwrap_or(0)
            .checked_add(1)
            .and_then(|v| i64::try_from(v).ok())
            .ok_or(Error::Bounds)?;
        tx.execute("INSERT INTO settings VALUES(1, ?1, ?2, ?3) ON CONFLICT(singleton) DO UPDATE SET revision=excluded.revision, value=excluded.value", params![account.bytes().as_slice(), revision, bytes])?;
        tx.commit()?;
        Ok(true)
    }
}
