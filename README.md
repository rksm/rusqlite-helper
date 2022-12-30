# rusqlite-helper

This is a helper that allows to manage SQLite tables.

Example:

```rust

pub fn setup_db(c: &rusqlite::Connection, force: bool) -> Result<(), rusqlite_helper::RusqliteHelperError> {
    let tables = rusqlite_helper::tables(c)?;
    types::Account::table().create(c, &tables, force)?;
    Ok(())
}


mod types {

    use chrono::prelude::*;
    use once_cell::sync::OnceCell;
    use rusqlite::{Connection, Params, Result};
    use serde::{Deserialize, Serialize};
    use rusqlite_helper::{InsertConflictResolution, Table};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Account {
        pub acct: String,
        pub id: Option<String>,
        pub name: String,
        pub display_name: String,
        pub note: String,
        pub url: String,
        pub fetched: DateTime<Utc>,
    }

    impl Account {
        pub fn table() -> &'static Table {
            static TABLE: OnceCell<Table> = OnceCell::new();
            TABLE.get_or_init(|| {
                Table::new(
                    "accounts",
                    "acct TEXT PRIMARY KEY,
                     id TEXT,
                     name TEXT NOT NULL,
                     display_name TEXT NOT NULL,
                     note TEXT NOT NULL,
                     url TEXT NOT NULL,
                     fetched TEXT NOT NULL",
                )
            })
        }

        pub fn load(c: &Connection, acct: &str) -> Result<Vec<Self>, rusqlite_helper::RusqliteHelperError> {
            Self::table().query(c, "WHERE acct = ?", [acct])
        }

        pub fn query(
            c: &Connection,
            where_query: &str,
            params: impl Params,
        ) -> Result<Vec<Self>, rusqlite_helper::RusqliteHelperError> {
            Self::table().query(c, where_query, params)
        }

        #[rustfmt::skip]
        pub fn insert(&self, db: &Connection) -> Result<usize, rusqlite_helper::RusqliteHelperError> {
            Self::table().insert(
                db, self, &["acct", "id", "name", "display_name", "note", "url", "fetched"],
                if self.id.is_some() { InsertConflictResolution::Replace } else { InsertConflictResolution::Ignore }
            )
        }
    }
}
```
