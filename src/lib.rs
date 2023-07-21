//! This is a helper that allows to manage SQLite tables.
//!
//! Example:
//!
//! ```rust
//!
//! pub fn setup_db(c: &rusqlite::Connection, force: bool) -> Result<(), rusqlite_helper::RusqliteHelperError> {
//!     let tables = rusqlite_helper::tables(c)?;
//!     types::Account::table().create(c, &tables, force)?;
//!     Ok(())
//! }
//!
//!
//! mod types {
//!
//!     use chrono::prelude::*;
//!     use once_cell::sync::OnceCell;
//!     use rusqlite::{Connection, Params, Result};
//!     use serde::{Deserialize, Serialize};
//!     use rusqlite_helper::{InsertConflictResolution, Table};
//!
//!     #[derive(Debug, Clone, Serialize, Deserialize)]
//!     pub struct Account {
//!         pub acct: String,
//!         pub id: Option<String>,
//!         pub name: String,
//!         pub display_name: String,
//!         pub note: String,
//!         pub url: String,
//!         pub fetched: DateTime<Utc>,
//!     }
//!
//!     impl Account {
//!         pub fn table() -> &'static Table {
//!             static TABLE: OnceCell<Table> = OnceCell::new();
//!             TABLE.get_or_init(|| {
//!                 Table::new(
//!                     "accounts",
//!                     "acct TEXT PRIMARY KEY,
//!                      id TEXT,
//!                      name TEXT NOT NULL,
//!                      display_name TEXT NOT NULL,
//!                      note TEXT NOT NULL,
//!                      url TEXT NOT NULL,
//!                      fetched TEXT NOT NULL",
//!                 )
//!             })
//!         }
//!
//!         pub fn load(c: &Connection, acct: &str) -> Result<Vec<Self>, rusqlite_helper::RusqliteHelperError> {
//!             Self::table().query(c, "WHERE acct = ?", [acct])
//!         }
//!
//!         pub fn query(
//!             c: &Connection,
//!             where_query: &str,
//!             params: impl Params,
//!         ) -> Result<Vec<Self>, rusqlite_helper::RusqliteHelperError> {
//!             Self::table().query(c, where_query, params)
//!         }
//!
//!         #[rustfmt::skip]
//!         pub fn insert(&self, db: &Connection) -> Result<usize, rusqlite_helper::RusqliteHelperError> {
//!             Self::table().insert(
//!                 db, self, &["acct", "id", "name", "display_name", "note", "url", "fetched"],
//!                 if self.id.is_some() { InsertConflictResolution::Replace } else { InsertConflictResolution::Ignore }
//!             )
//!         }
//!     }
//! }
//! ```

#[macro_use]
extern crate log;

use rusqlite::Connection;
use serde_rusqlite::to_params_named;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RusqliteHelperError {
    #[error("SQLite error {0}")]
    SQLite(#[from] rusqlite::Error),
    #[error("Serialization error {0}")]
    Serialization(#[from] serde_rusqlite::Error),
}

pub fn tables(c: &Connection) -> Result<HashSet<String>, RusqliteHelperError> {
    // 1: schema
    // 2: (table) name
    // 3: type
    let mut tables = HashSet::new();
    c.pragma_query(None, "table_list", |row| {
        let name: String = row.get(1)?;
        let ty: String = row.get(2)?;
        if ty == "table" {
            tables.insert(name);
        }
        Ok(())
    })?;

    Ok(tables)
}

pub struct Table {
    pub name: String,
    pub def: String,
}

#[allow(unused)]
#[derive(Default, Clone)]
pub enum InsertConflictResolution<'a> {
    #[default]
    None,
    Ignore,
    Abort,
    Replace,
    Upsert(&'a str),
}

impl Table {
    pub fn new(name: impl ToString, def: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            def: def.to_string(),
        }
    }

    pub fn create(
        &self,
        c: &Connection,
        tables: &HashSet<String>,
        force: bool,
    ) -> Result<(), RusqliteHelperError> {
        let Self { name, def } = self;
        let exists = tables.contains(name);
        let create = !exists || force;
        if create {
            if exists {
                info!("dropping table {name}");
                c.execute(&(format!("DROP TABLE {name};")), ())?;
            }
            info!("creating table {name}");
            c.execute(&format!("CREATE TABLE {name} ({def})"), ())?;
        }
        Ok(())
    }

    /// Insert self into the database, return true if the row was inserted or
    /// updated, false if ignored.
    pub fn insert(
        &self,
        c: &Connection,
        row: impl serde::Serialize,
        fields: &[&str],
        conflict: InsertConflictResolution<'_>,
    ) -> Result<bool, RusqliteHelperError> {
        let Self { name, .. } = self;
        let values = {
            let mut values = fields.join(", :");
            values.insert(0, ':');
            values
        };
        let fields = fields.join(",");
        let sql = match conflict {
            InsertConflictResolution::None => {
                format!("INSERT INTO {name} ({fields}) VALUES ({values})")
            }
            InsertConflictResolution::Ignore => {
                format!("INSERT OR IGNORE INTO {name} ({fields}) VALUES ({values})")
            }
            InsertConflictResolution::Abort => {
                format!("INSERT OR ABORT INTO {name} ({fields}) VALUES ({values})")
            }
            InsertConflictResolution::Replace => {
                format!("INSERT OR REPLACE INTO {name} ({fields}) VALUES ({values})")
            }
            InsertConflictResolution::Upsert(on_conflict) => {
                format!("INSERT INTO {name} ({fields}) VALUES ({values}) {on_conflict}")
            }
        };
        trace!("{sql}");
        let n = c.execute(&sql, to_params_named(row).unwrap().to_slice().as_slice())?;
        Ok(n != 0)
    }

    pub fn query<D: serde::de::DeserializeOwned>(
        &self,
        c: &Connection,
        where_stmt: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<D>, RusqliteHelperError> {
        let Self { name, .. } = self;
        let mut stmt = c.prepare(&(format!("SELECT * FROM {name} {where_stmt};")))?;
        let rows = stmt.query_and_then(params, serde_rusqlite::from_row::<D>)?;
        Ok(rows.collect::<Result<Vec<D>, _>>()?)
    }
}
