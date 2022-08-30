use std::path::Path;

use rusqlite::{Connection, Error as DbError};

use crate::{mmr, NumberScope};

pub(crate) struct Storage {
    conn: Connection,
}

impl Storage {
    pub(crate) fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let conn = Connection::open(path).expect("db open");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id    INTEGER PRIMARY KEY,
                left  INTEGER NOT NULL,
                right INTEGER NOT NULL
             )",
            (),
        )
        .expect("db init");
        println!("storage ok");
        Self { conn }
    }

    pub(crate) fn get_max(&self) -> Option<u64> {
        self.conn
            .prepare("SELECT max(right) FROM nodes")
            .expect("db prepare")
            .query_map([], |row| {
                let value = row.get_ref(0)?;
                if value == rusqlite::types::ValueRef::Null {
                    Ok(None)
                } else {
                    Ok(Some(row.get(0)?))
                }
            })
            .expect("db query")
            .next()
            .transpose()
            .expect("db data convert")
            .flatten()
    }

    fn get_node(&self, pos: u64) -> Option<NumberScope> {
        self.conn
            .prepare("SELECT left, right FROM nodes where id = ?")
            .expect("db prepare")
            .query_map([pos], |row| {
                Ok(NumberScope {
                    start: row.get(0)?,
                    end: row.get(1)?,
                })
            })
            .expect("db query")
            .next()
            .transpose()
            .expect("db data convert")
    }

    fn set_node(&self, pos: u64, elem: &NumberScope) -> Result<usize, DbError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO nodes (id, left, right) VALUES (?1, ?2, ?3)",
            (&pos, &elem.start, &elem.end),
        )
    }
}

impl mmr::MMRStore<NumberScope> for &Storage {
    fn get_elem(&self, pos: u64) -> mmr::Result<Option<NumberScope>> {
        Ok(self.get_node(pos))
    }
    fn append(&mut self, pos: u64, elems: Vec<NumberScope>) -> mmr::Result<()> {
        for (offset, elem) in elems.iter().enumerate() {
            let pos: u64 = pos + (offset as u64);
            self.set_node(pos, elem).map_err(|err| {
                mmr::Error::StoreError(format!("Failed to append to MMR, DB error {}", err))
            })?;
        }
        Ok(())
    }
}
