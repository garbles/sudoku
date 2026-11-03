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
const TEST_SEEDS: std::ops::RangeInclusive<i64> = 9_000_000_001..=9_000_000_014;

/// A result before the database gives it an id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewResult {
    pub finished_at: i64,
    pub mode: u8,
    pub givens: u8,
    pub seconds: u64,
    pub seed: u64,
    /// The version of the generator that made the puzzle (PUZ-5).
    pub generator: u8,
}

pub struct ResultsDb {
    conn: Connection,
}

/// The version of the schema (`PRAGMA user_version`). Version 1 (user_version 0) had the modes 0..4
/// and no generator version.
const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = "CREATE TABLE results (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    finished_at INTEGER NOT NULL,
    mode        INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 6),
    givens      INTEGER NOT NULL CHECK (givens BETWEEN 0 AND 81),
    seconds     INTEGER NOT NULL CHECK (seconds >= 0),
    seed        INTEGER NOT NULL,
    generator   INTEGER NOT NULL CHECK (generator >= 1)
);";

/// Changes a results table of version 1 to version 2, in one transaction. The rows that agree with
/// ANA-1 go to the new table with generator version 1, with the same id. The rows that the app cannot
/// use stay in the file, in the table `results_v1` (ANA-1). If there are no such rows, the app removes
/// that table.
fn migrate_to_2(conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(&format!(
        "ALTER TABLE results RENAME TO results_v1;
         {SCHEMA}
         INSERT INTO results (id, finished_at, mode, givens, seconds, seed, generator)
             SELECT id, finished_at, mode, givens, seconds, seed, 1 FROM results_v1
             WHERE typeof(finished_at) = 'integer' AND typeof(seed) = 'integer'
               AND typeof(mode) = 'integer' AND mode BETWEEN 0 AND 4
               AND typeof(givens) = 'integer' AND givens BETWEEN 0 AND 81
               AND typeof(seconds) = 'integer' AND seconds >= 0;
         DELETE FROM results_v1 WHERE id IN (SELECT id FROM results);"
    ))?;
    let left: i64 = tx.query_row("SELECT count(*) FROM results_v1", [], |r| r.get(0))?;
    if left == 0 {
        tx.execute_batch("DROP TABLE results_v1;")?;
    }
    tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
    tx.commit()
}

impl ResultsDb {
    pub fn open(path: &Path) -> rusqlite::Result<ResultsDb> {
        ResultsDb::init(Connection::open(path)?)
    }

    pub fn in_memory() -> ResultsDb {
        ResultsDb::init(Connection::open_in_memory().expect("in-memory database")).expect("schema")
    }

    fn init(mut conn: Connection) -> rusqlite::Result<ResultsDb> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let exists: bool =
            conn.query_row("SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'results'", [], |r| r.get(0))?;
        if !exists {
            conn.execute_batch(&format!("{SCHEMA} PRAGMA user_version = {SCHEMA_VERSION};"))?;
        } else if version < SCHEMA_VERSION {
            // The app can still open the file if the migration fails. It tries again the next time.
            if let Err(e) = migrate_to_2(&mut conn) {
                eprintln!("sudoku: cannot change the results database to version {SCHEMA_VERSION}: {e}");
            }
        }
        Ok(ResultsDb { conn })
    }

    pub fn insert(&mut self, r: &NewResult) -> rusqlite::Result<ResultRow> {
        self.conn.execute(
            "INSERT INTO results (finished_at, mode, givens, seconds, seed, generator) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![r.finished_at, r.mode, r.givens, r.seconds as i64, r.seed as i64, r.generator],
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
                let (seed, mode) = (TEST_SEEDS.start() + i - 1, (i % crate::generator::MODES as i64) as u8);
                let givens = crate::generator::generate(seed as u64, mode).givens_count() as i64;
                tx.execute(
                    "INSERT INTO results (finished_at, mode, givens, seconds, seed, generator) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![1_792_800_000 - i * 86_400 - i * 1234, mode, givens, 300 + i * 97, seed, crate::generator::VERSION],
                )?;
            }
            14
        } else {
            removed
        };
        tx.commit()?;
        Ok(n)
    }

    /// Removes all results ("Delete all games", ANA-12).
    pub fn delete_all(&mut self) -> rusqlite::Result<usize> {
        self.conn.execute("DELETE FROM results", [])
    }

    pub fn delete(&mut self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM results WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// All results, with the newest first. The app does not use a row that does not agree with ANA-1
    /// (for example a database from a different schema). The row stays in the file.
    pub fn all(&self) -> rusqlite::Result<Vec<ResultRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, finished_at, mode, givens, seconds, seed FROM results ORDER BY finished_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let (mode, givens, seconds): (i64, i64, i64) = (row.get(2)?, row.get(3)?, row.get(4)?);
            if !((0..crate::generator::MODES as i64).contains(&mode) && (0..=81).contains(&givens) && seconds >= 0) {
                return Ok(None);
            }
            Ok(Some(ResultRow {
                id: row.get(0)?,
                finished_at: row.get(1)?,
                mode: mode as u8,
                givens: givens as u8,
                seconds: seconds as u64,
                seed: row.get::<_, i64>(5)? as u64,
            }))
        })?;
        // A value of the wrong type (for example text in `mode`) also makes the row not usable.
        Ok(rows.filter_map(|r| r.ok().flatten()).collect())
    }
}

/// The folder for the saved data. `SUDOKU_DATA_DIR` changes it. The corpus build has a different
/// folder (`corpus::DATA_DIR_NAME`), thus it never changes the data of the player.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("SUDOKU_DATA_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/home/root"));
    #[cfg(feature = "corpus")]
    let name = crate::corpus::DATA_DIR_NAME;
    #[cfg(not(feature = "corpus"))]
    let name = "rm-sudoku";
    home.join(".local/share").join(name)
}

/// Writes JSON to a file. It writes a temporary file, syncs it and then renames it, so that a power
/// loss never leaves a partial file.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    write_atomic(path, &serde_json::to_vec(value).map_err(std::io::Error::other)?)
}

/// Writes `data` to a temporary file, syncs it and renames it to `path` (refer to `save_json`).
fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    // Sync the folder too, so that the rename stays after a power loss.
    if let Some(dir) = path.parent() {
        fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

/// Writes JSON files on a background thread, each with `save_json`: the sync to the disk (eMMC)
/// does not delay the main loop (the screen and the ink). The writes keep their order. A newer
/// write of a file replaces a write of the same file that waits. `flush` waits for all writes.
pub struct Saver {
    tx: Option<std::sync::mpsc::Sender<SaverMsg>>,
    failed: std::sync::mpsc::Receiver<PathBuf>,
    thread: Option<std::thread::JoinHandle<()>>,
}

enum SaverMsg {
    Write(PathBuf, Vec<u8>),
    /// Sends true when all earlier writes are complete and correct.
    Flush(std::sync::mpsc::Sender<bool>),
}

impl Default for Saver {
    fn default() -> Saver {
        Saver::new()
    }
}

impl Saver {
    pub fn new() -> Saver {
        let (tx, rx) = std::sync::mpsc::channel::<SaverMsg>();
        let (failed_tx, failed) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            // The writes that wait, in order. A newer write of the same file replaces the older one.
            let mut waiting: Vec<(PathBuf, Vec<u8>)> = Vec::new();
            let mut all_ok = true;
            let write_all = |waiting: &mut Vec<(PathBuf, Vec<u8>)>, all_ok: &mut bool| {
                for (path, data) in waiting.drain(..) {
                    if let Err(e) = write_atomic(&path, &data) {
                        eprintln!("sudoku: cannot save {}: {e}", path.display());
                        *all_ok = false;
                        let _ = failed_tx.send(path);
                    }
                }
            };
            while let Ok(msg) = rx.recv() {
                let mut next = Some(msg);
                while let Some(msg) = next.take() {
                    match msg {
                        SaverMsg::Write(path, data) => {
                            waiting.retain(|(p, _)| *p != path);
                            waiting.push((path, data));
                        }
                        SaverMsg::Flush(done) => {
                            write_all(&mut waiting, &mut all_ok);
                            let _ = done.send(std::mem::replace(&mut all_ok, true));
                        }
                    }
                    next = rx.try_recv().ok();
                }
                write_all(&mut waiting, &mut all_ok);
            }
            write_all(&mut waiting, &mut all_ok);
        });
        Saver { tx: Some(tx), failed, thread: Some(thread) }
    }

    /// Writes `value` to `path` later (the JSON is made now).
    pub fn write<T: Serialize>(&self, path: &Path, value: &T) -> std::io::Result<()> {
        let data = serde_json::to_vec(value).map_err(std::io::Error::other)?;
        self.send(SaverMsg::Write(path.to_path_buf(), data))
    }

    /// Waits until all earlier writes are on the disk. True if all writes since the last flush are
    /// correct.
    pub fn flush(&self) -> bool {
        let (done, wait) = std::sync::mpsc::channel();
        self.send(SaverMsg::Flush(done)).is_ok() && wait.recv().unwrap_or(false)
    }

    /// The files of the writes that failed since the last call.
    pub fn failures(&self) -> Vec<PathBuf> {
        self.failed.try_iter().collect()
    }

    fn send(&self, msg: SaverMsg) -> std::io::Result<()> {
        let tx = self.tx.as_ref().expect("the saver is open");
        tx.send(msg).map_err(|_| std::io::Error::other("the save thread stopped"))
    }
}

impl Drop for Saver {
    /// Writes all files that wait, then stops the thread.
    fn drop(&mut self) {
        drop(self.tx.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let data = fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sudoku-saver-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_saver_writes_the_last_value_of_each_file() {
        // PER-1, PER-7: the background writes keep their order; after a flush the files have the
        // last values.
        let dir = temp_dir("order");
        let saver = Saver::new();
        for i in 0..200u32 {
            saver.write(&dir.join("a.json"), &i).unwrap();
            saver.write(&dir.join("b.json"), &(1000 + i)).unwrap();
        }
        assert!(saver.flush());
        assert_eq!(load_json::<u32>(&dir.join("a.json")), Some(199));
        assert_eq!(load_json::<u32>(&dir.join("b.json")), Some(1199));
        saver.write(&dir.join("a.json"), &7u32).unwrap();
        // The drop writes what waits.
        drop(saver);
        assert_eq!(load_json::<u32>(&dir.join("a.json")), Some(7));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_saver_gives_the_failed_files() {
        // A file in a folder that is a file cannot be written: the main loop gets the path, and
        // marks the data dirty again.
        let dir = temp_dir("fail");
        fs::write(dir.join("not-a-folder"), b"x").unwrap();
        let bad = dir.join("not-a-folder").join("game.json");
        let saver = Saver::new();
        saver.write(&bad, &1u32).unwrap();
        assert!(!saver.flush());
        assert_eq!(saver.failures(), vec![bad]);
        saver.write(&dir.join("ok.json"), &2u32).unwrap();
        assert!(saver.flush(), "a flush gives the result of the writes after the last flush");
        assert!(saver.failures().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn insert_list_delete() {
        let mut db = ResultsDb::in_memory();
        let a = db.insert(&NewResult { finished_at: 100, mode: 1, givens: 37, seconds: 600, seed: u64::MAX, generator: 2 }).unwrap();
        let b = db.insert(&NewResult { finished_at: 200, mode: 4, givens: 24, seconds: 4000, seed: 5, generator: 2 }).unwrap();
        let all = db.all().unwrap();
        assert_eq!(all, vec![b.clone(), a.clone()]);
        assert_eq!(all[1].seed, u64::MAX);
        db.delete(b.id).unwrap();
        assert_eq!(db.all().unwrap(), vec![a]);
    }

    #[test]
    fn test_results_are_added_and_removed_exactly() {
        let mut db = ResultsDb::in_memory();
        let mine = db.insert(&NewResult { finished_at: 5, mode: 2, givens: 31, seconds: 60, seed: 77, generator: 2 }).unwrap();
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
            db.insert(&NewResult { finished_at: 1, mode: 0, givens: 44, seconds: 5, seed: 9, generator: 2 }).unwrap();
        }
        assert_eq!(ResultsDb::open(&path).unwrap().all().unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A results file of version 1, with the schema of version 1 (or with no CHECK constraints).
    fn version_1_db(path: &Path, rows: &str, checks: bool) {
        let conn = Connection::open(path).unwrap();
        let check = |c: &str| if checks { c.to_string() } else { String::new() };
        conn.execute_batch(&format!(
            "CREATE TABLE results (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 finished_at INTEGER NOT NULL,
                 mode        INTEGER NOT NULL {},
                 givens      INTEGER NOT NULL {},
                 seconds     INTEGER NOT NULL {},
                 seed        INTEGER NOT NULL
             );
             INSERT INTO results (id, finished_at, mode, givens, seconds, seed) VALUES {rows};",
            check("CHECK (mode BETWEEN 0 AND 4)"),
            check("CHECK (givens BETWEEN 0 AND 81)"),
            check("CHECK (seconds >= 0)"),
        ))
        .unwrap();
    }

    fn temp_db(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("sudoku-db-{name}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("results.sqlite");
        (dir, path)
    }

    #[test]
    fn version_1_results_are_migrated() {
        // ANA-1, PER-4: the results of version 1 stay, with the same id, mode, time and seed. They
        // have generator version 1. A new result can have mode 5 or 6.
        let (dir, path) = temp_db("migrate");
        version_1_db(&path, "(3, 100, 0, 44, 60, 7), (8, 200, 4, 24, 900, 8)", true);
        let mut db = ResultsDb::open(&path).unwrap();
        let ids: Vec<(i64, u8, u64, u64)> = db.all().unwrap().iter().map(|r| (r.id, r.mode, r.seconds, r.seed)).collect();
        assert_eq!(ids, vec![(8, 4, 900, 8), (3, 0, 60, 7)]);
        let generators: Vec<i64> =
            db.conn.prepare("SELECT generator FROM results").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(generators, vec![1, 1]);
        let tables: i64 = db.conn.query_row("SELECT count(*) FROM sqlite_master WHERE name = 'results_v1'", [], |r| r.get(0)).unwrap();
        assert_eq!(tables, 0, "no row stays in the old table, thus the app removes it");
        let new = db.insert(&NewResult { finished_at: 300, mode: 6, givens: 24, seconds: 5, seed: 9, generator: 2 }).unwrap();
        assert!(new.id > 8, "a new id is not the id of an old result");
        drop(db);
        // The migration is done one time only.
        assert_eq!(ResultsDb::open(&path).unwrap().all().unwrap().len(), 3);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn version_1_rows_that_break_ana_1_stay_in_the_file() {
        // ANA-1: the row stays in the file. Mode 5 is not possible in version 1.
        let (dir, path) = temp_db("migrate-bad");
        version_1_db(&path, "(1, 100, 1, 40, 60, 11), (2, 100, 5, 40, 60, 12), (3, 100, 'x', 40, 60, 13)", false);
        let db = ResultsDb::open(&path).unwrap();
        assert_eq!(db.all().unwrap().iter().map(|r| r.seed).collect::<Vec<_>>(), vec![11]);
        let kept: Vec<i64> =
            db.conn.prepare("SELECT seed FROM results_v1 ORDER BY id").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(kept, vec![12, 13]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
