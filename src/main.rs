use clap::Parser as ClapParser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

enum DbPool {
    Sqlite(sqlx::sqlite::SqlitePool),
    Pg(sqlx::postgres::PgPool),
}

use sqlx::{FromRow, Row};
#[derive(Debug, FromRow, Clone)]
struct Ticket {
    id: i64,
    name: String,
}

macro_rules! xdb {
    ($db:ident, $pool:ident, $tree: tt) => {
        match &$db {
            DbPool::Sqlite($pool) => $tree,
            DbPool::Pg($pool) => $tree,
        }
    };
}

/// This program does something useful, but its author needs to edit this.
/// Else it will be just hanging around forever
#[derive(Debug, Clone, ClapParser, Serialize, Deserialize)]
#[clap(version = env!("GIT_VERSION"), author = "Andrew Yourtchenko <ayourtch@gmail.com>")]
struct Opts {
    /// Target hostname to do things on
    #[clap(short, long, default_value = "localhost")]
    target_host: String,

    /// Database path
    #[clap(short)]
    db: String,

    /// Override options from this yaml/json file
    #[clap(short, long)]
    options_override: Option<String>,

    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,
}

async fn async_main(opts: Opts) -> Option<i32> {
    use sqlx::Row;
    println!("Hello, world!");
    let db = if opts.db.starts_with("sqlite://") {
        let p = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&opts.db)
            .await
            .ok()?;
        DbPool::Sqlite(p)
    } else if opts.db.starts_with("postgres://") {
        let p = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&opts.db)
            .await
            .ok()?;
        DbPool::Pg(p)
    } else {
        panic!("Need a database path");
    };
    println!("Connected to a db");

    match &db {
        DbPool::Sqlite(pool) => {
            sqlx::query(
                r#"
CREATE TABLE IF NOT EXISTS ticket (
  id integer primary key autoincrement,
  name text
);"#,
            )
            .execute(pool)
            .await
            .unwrap();
        }
        DbPool::Pg(pool) => {
            sqlx::query(
                r#"
CREATE TABLE IF NOT EXISTS ticket (
  id bigserial,
  name text
);"#,
            )
            .execute(pool)
            .await
            .unwrap();
        }
    };

    let row = match &db {
        DbPool::Sqlite(pool) => {
            let row = sqlx::query_as("insert into ticket (name) values ($1) returning id")
                .bind("a new ticket")
                .fetch_one(pool)
                .await
                .unwrap();
            row
        }
        DbPool::Pg(pool) => {
            let row: (i64,) = sqlx::query_as("insert into ticket (name) values ($1) returning id")
                .bind("a new ticket")
                .fetch_one(pool)
                .await
                .unwrap();
            row
        }
    };
    println!("Row: {:?}", row);

    xdb!(db, pool, {
        let rows = sqlx::query("SELECT * FROM ticket")
            .fetch_all(pool)
            .await
            .unwrap();
        let str_result = rows
            .iter()
            .map(|r| format!("{} - {}", r.get::<i64, _>("id"), r.get::<String, _>("name")))
            .collect::<Vec<String>>()
            .join(", ");
        println!("\n== select tickets:\n{}", str_result);
    });

    let tickets: Vec<Ticket> = xdb!(db, pool, {
        let select_query = sqlx::query_as::<_, Ticket>("SELECT id, name FROM ticket");
        select_query.fetch_all(pool).await.unwrap()
    });
    println!("tickets: {:?}", &tickets);

    Some(0)
}

fn main() {
    let opts: Opts = Opts::parse();

    // allow to load the options, so far there is no good built-in way
    let opts = if let Some(fname) = &opts.options_override {
        if let Ok(data) = std::fs::read_to_string(&fname) {
            let res = serde_json::from_str(&data);
            if res.is_ok() {
                res.unwrap()
            } else {
                serde_yaml::from_str(&data).unwrap()
            }
        } else {
            opts
        }
    } else {
        opts
    };

    if opts.verbose > 4 {
        let data = serde_json::to_string_pretty(&opts).unwrap();
        println!("{}", data);
        println!("===========");
        let data = serde_yaml::to_string(&opts).unwrap();
        println!("{}", data);
    }

    println!("Hello, here is your options: {:#?}", &opts);

    async_std::task::block_on(async_main(opts));
}
