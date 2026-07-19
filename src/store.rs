use std::{path::Path, sync::Mutex};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};

use crate::domain::RunResult;

pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS results (
                run_id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                payload TEXT NOT NULL,
                human_score INTEGER
            );",
        )?;
        Ok(Self { connection: Mutex::new(connection) })
    }

    pub fn save(&self, result: &RunResult) -> Result<()> {
        let payload = serde_json::to_string(result)?;
        self.connection.lock().map_err(|_| anyhow::anyhow!("The database lock failed"))?.execute(
            "INSERT INTO results (run_id, created_at, payload, human_score) VALUES (?1, ?2, ?3, ?4)",
            params![result.run_id, result.created_at, payload, result.human_quality_score],
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<RunResult>> {
        let connection = self.connection.lock().map_err(|_| anyhow::anyhow!("The database lock failed"))?;
        let mut statement = connection.prepare("SELECT payload, human_score FROM results ORDER BY created_at DESC")?;
        let rows = statement.query_map([], |row| {
            let payload: String = row.get(0)?;
            let score: Option<u8> = row.get(1)?;
            Ok((payload, score))
        })?;
        let mut results = Vec::new();
        for row in rows {
            let (payload, score) = row?;
            let mut result: RunResult = serde_json::from_str(&payload).context("A stored result is invalid")?;
            result.human_quality_score = score;
            results.push(result);
        }
        Ok(results)
    }

    pub fn review(&self, run_id: &str, score: u8) -> Result<()> {
        if !(1..=5).contains(&score) {
            bail!("The human score must be from 1 to 5");
        }
        let changed = self.connection.lock().map_err(|_| anyhow::anyhow!("The database lock failed"))?.execute(
            "UPDATE results SET human_score = ?1 WHERE run_id = ?2",
            params![score, run_id],
        )?;
        if changed == 0 {
            bail!("The result does not exist");
        }
        Ok(())
    }
}
