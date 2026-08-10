use rusqlite::params;

use crate::database::{DbError, FolderRecord, PlayTuneDb};

impl PlayTuneDb {
    // Folders
    pub fn add_folder(&self, path: &str, name: &str, track_count: i32) -> Result<i64, DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO folders (path, name, track_count) VALUES (?, ?, ?) ON CONFLICT(path) DO UPDATE SET name = excluded.name",
            params![path, name, track_count],
        )?;
        let mut stmt = conn.prepare_cached("SELECT id FROM folders WHERE path = ?")?;
        let id = stmt.query_row(params![path], |row| row.get(0))?;
        Ok(id)
    }

    pub fn get_all_folders(&self) -> Result<Vec<FolderRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT id, path, name, (SELECT COUNT(*) FROM tracks WHERE tracks.folder_id = folders.id), IFNULL(added_at, '') FROM folders ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(FolderRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                track_count: row.get(3)?,
                added_at: row.get(4)?,
            })
        })?;
        let mut folders = Vec::new();
        for f in rows {
            folders.push(f?);
        }
        Ok(folders)
    }

    pub fn delete_folder(&self, id: i64) -> Result<(), DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM tracks WHERE folder_id = ?", params![id])?;
        tx.execute("DELETE FROM folders WHERE id = ?", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_track(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM tracks WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Run a closure with a transaction. The closure receives a
    /// `&Transaction` and can execute multiple SQL statements; all are
    /// committed atomically on success, or rolled back on error.
    ///
    /// # Performance
    ///
    /// Wrapping a batch of inserts/updates in a single transaction is
    /// critical on HDD: without an explicit transaction, SQLite wraps
    /// every individual statement in its own implicit transaction, and
    /// each commit forces a write to the WAL file. For a 1000-track
    /// library scan, that's 1000 WAL writes vs. 1 — a 10-100x speedup
    /// on HDD where each write involves a seek.
    pub fn with_transaction<F, R>(&self, f: F) -> Result<R, DbError>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<R, DbError>,
    {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}
