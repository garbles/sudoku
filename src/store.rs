//! Data that the app keeps (REQUIREMENTS.md §14): the results database (SQLite), the settings and
//! the game in progress.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};

/// One completed puzzle (§14.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultRow {
    pub id: i64,
    /// Unix time (seconds) of completion.
    pub finished_at: i64,
    pub mode: u8,
    pub givens: u8,
    pub seconds: u64,
    pub seed: u64,
}

/// The file of the results database in the data folder (`data_dir`).
pub const RESULTS_FILE: &str = "results.sqlite";

/// The seeds of the test results (`ResultsDb::test_results`). The generator never makes them for a
/// player, because a player's seed is a random 64-bit value.
pub const TEST_SEEDS: std::ops::RangeInclusive<i64> = 9_000_000_001..=9_000_000_014;

/// A result before the database gives it an id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewResult {
    pub finished_at: i64,
    pub mode: u8,
    pub givens: u8,
    pub seconds: u64,
    pub seed: u64,
}

pub struct ResultsDb {
    conn: Connection,
}

impl ResultsDb {
    pub fn open(path: &Path) -> rusqlite::Result<ResultsDb> {
        ResultsDb::init(Connection::open(path)?)
    }

    pub fn in_memory() -> ResultsDb {
        ResultsDb::init(Connection::open_in_memory().expect("in-memory database")).expect("schema")
    }

    fn init(conn: Connection) -> rusqlite::Result<ResultsDb> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             CREATE TABLE IF NOT EXISTS results (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 finished_at INTEGER NOT NULL,
                 mode        INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 4),
                 givens      INTEGER NOT NULL CHECK (givens BETWEEN 0 AND 81),
                 seconds     INTEGER NOT NULL CHECK (seconds >= 0),
                 seed        INTEGER NOT NULL
             );",
        )?;
        Ok(ResultsDb { conn })
    }

    pub fn insert(&mut self, r: &NewResult) -> rusqlite::Result<ResultRow> {
        self.conn.execute(
            "INSERT INTO results (finished_at, mode, givens, seconds, seed) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![r.finished_at, r.mode, r.givens, r.seconds as i64, r.seed as i64],
        )?;
        Ok(ResultRow {
            id: self.conn.last_insert_rowid(),
            finished_at: r.finished_at,
            mode: r.mode,
            givens: r.givens,
            seconds: r.seconds,
            seed: r.seed,
        })
    }

    /// Adds (true) or removes (false) the test results: 14 rows with the seeds TEST_SEEDS, in one
    /// transaction. An add first removes the test results that are already there, so the database
    /// never has two copies. Only an examination of the games list on a device uses them.
    pub fn test_results(&mut self, add: bool) -> rusqlite::Result<usize> {
        let tx = self.conn.transaction()?;
        let removed =
            tx.execute("DELETE FROM results WHERE seed BETWEEN ?1 AND ?2", params![TEST_SEEDS.start(), TEST_SEEDS.end()])?;
        let n = if add {
            for i in 1..=14i64 {
                // The quantity of starting numbers of the puzzle of this seed and mode (PUZ-5, CMP-5).
                let (seed, mode) = (TEST_SEEDS.start() + i - 1, (i % 5) as u8);
                let givens = crate::generator::generate(seed as u64, mode).givens_count() as i64;
                tx.execute(
                    "INSERT INTO results (finished_at, mode, givens, seconds, seed) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![1_792_800_000 - i * 86_400 - i * 1234, mode, givens, 300 + i * 97, seed],
                )?;
            }
            14
        } else {
            removed
        };
        tx.commit()?;
        Ok(n)
    }

    pub fn delete(&mut self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM results WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// All results, with the newest first.
    pub fn all(&self) -> rusqlite::Result<Vec<ResultRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, finished_at, mode, givens, seconds, seed FROM results ORDER BY finished_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ResultRow {
                id: row.get(0)?,
                finished_at: row.get(1)?,
                mode: row.get(2)?,
                givens: row.get(3)?,
                seconds: row.get::<_, i64>(4)? as u64,
                seed: row.get::<_, i64>(5)? as u64,
            })
        })?;
        rows.collect()
    }
}

/// The folder for the saved data. `SUDOKU_DATA_DIR` changes it.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("SUDOKU_DATA_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/home/root"));
    home.join(".local/share/rm-sudoku")
}

/// Writes JSON to a file. It writes a temporary file, syncs it and then renames it, so that a power
/// loss never leaves a partial file.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let data = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    // Sync the folder too, so that the rename stays after a power loss.
    if let Some(dir) = path.parent() {
        fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

pub fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let data = fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_list_delete() {
        let mut db = ResultsDb::in_memory();
        let a = db.insert(&NewResult { finished_at: 100, mode: 1, givens: 37, seconds: 600, seed: u64::MAX }).unwrap();
        let b = db.insert(&NewResult { finished_at: 200, mode: 4, givens: 24, seconds: 4000, seed: 5 }).unwrap();
        let all = db.all().unwrap();
        assert_eq!(all, vec![b.clone(), a.clone()]);
        assert_eq!(all[1].seed, u64::MAX);
        db.delete(b.id).unwrap();
        assert_eq!(db.all().unwrap(), vec![a]);
    }

    #[test]
    fn test_results_are_added_and_removed_exactly() {
        let mut db = ResultsDb::in_memory();
        let mine = db.insert(&NewResult { finished_at: 5, mode: 2, givens: 31, seconds: 60, seed: 77 }).unwrap();
        assert_eq!(db.test_results(true).unwrap(), 14);
        assert_eq!(db.test_results(true).unwrap(), 14);
        assert_eq!(db.all().unwrap().len(), 15, "a second add does not make copies");
        assert_eq!(db.test_results(false).unwrap(), 14);
        assert_eq!(db.all().unwrap(), vec![mine], "only the test rows go");
    }

    #[test]
    fn database_keeps_results_after_reopen() {
        // PER-4
        let dir = std::env::temp_dir().join(format!("sudoku-db-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("results.sqlite");
        {
            let mut db = ResultsDb::open(&path).unwrap();
            db.insert(&NewResult { finished_at: 1, mode: 0, givens: 44, seconds: 5, seed: 9 }).unwrap();
        }
        assert_eq!(ResultsDb::open(&path).unwrap().all().unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
