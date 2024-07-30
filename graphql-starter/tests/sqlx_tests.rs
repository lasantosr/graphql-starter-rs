use std::{borrow::Cow, sync::LazyLock, time::Duration};

use anyhow::Context;
use chrono::NaiveDateTime;
use graphql_starter::{
    error::Result,
    pagination::{BackwardPageQuery, ForwardPageQuery, PageQuery},
    sqlx_query_paginated_as,
};
use sqlx::{
    migrate::{Migration, MigrationType, Migrator},
    postgres::PgPoolOptions,
    PgPool,
};

#[allow(unused)]
struct TodoRow {
    id: i32,
    timestamp: NaiveDateTime,
    item: String,
}

static MIGRATIONS: LazyLock<Migrator> = LazyLock::new(migrations);

fn migrations() -> Migrator {
    let mut migrations = Vec::new();

    migrations.push(Migration {
        version: 0,
        description: Cow::Borrowed("create table"),
        migration_type: MigrationType::Simple,
        sql: Cow::Borrowed(
            r#"
            CREATE TABLE "todo" (
                "id" serial PRIMARY KEY,
                "timestamp" timestamp NOT NULL,
                "item" varchar(250) NOT NULL
            );
        "#,
        ),
        checksum: Cow::Owned(vec![1]),
    });

    for n in 1..=100 {
        migrations.push(Migration {
            version: n,
            description: Cow::Owned(format!("insert item #{n}")),
            migration_type: MigrationType::Simple,
            sql: Cow::Owned(format!(
                r#"INSERT INTO "todo" ("timestamp", "item") VALUES ('2023-01-01 10:00:00.{n:0>3}', 'item #{n}')"#
            )),
            checksum: Cow::Owned(vec![1]),
        });
    }

    Migrator {
        migrations: Cow::Owned(migrations),
        ignore_missing: false,
        locking: true,
    }
}

#[ignore = "just used to setup a sample database to write the tests"]
#[tokio::test]
async fn setup_database() -> anyhow::Result<()> {
    // Setup migrations
    let migrator = migrations();

    // Setup the database pool
    let pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect("postgres://postgres:postgres@localhost:5432/postgres")
        .await?;

    // Run migrations
    migrator.run(&pool).await.context("Couldn't apply migrations")
}

#[sqlx::test(migrator = "MIGRATIONS")]
async fn test_start(pool: PgPool) -> Result<()> {
    // tracing_subscriber::fmt().compact().with_env_filter("trace").init();

    // Retrieve the first 10 rows
    let page = PageQuery::Forward(ForwardPageQuery { first: 10, after: None });

    let res = sqlx_query_paginated_as!(
        page, &pool,
        [timestamp.asc(): NaiveDateTime, id.desc(): i32],
        TodoRow,
        r#"SELECT * FROM "todo" WHERE "id" != $1 AND "id" != $2"#,
        50i32,
        60i32
    );

    assert!(res.page_info.has_next_page);
    assert!(!res.page_info.has_previous_page);

    let rows = res.into_iter().collect::<Vec<_>>();

    assert_eq!(10, rows.len());
    assert_eq!(1, rows.first().unwrap().node.id);
    assert_eq!(10, rows.get(9).unwrap().node.id);

    // Retrieve next 10 rows
    let page = PageQuery::Forward(ForwardPageQuery {
        first: 10,
        after: Some(rows.last().unwrap().cursor.clone()),
    });

    let res = sqlx_query_paginated_as!(
        page, &pool,
        [timestamp.asc(): NaiveDateTime, id.desc(): i32],
        TodoRow,
        r#"SELECT * FROM "todo""#
    );

    assert!(res.page_info.has_next_page);

    let rows = res.into_iter().collect::<Vec<_>>();

    assert_eq!(10, rows.len());
    assert_eq!(11, rows.first().unwrap().node.id);
    assert_eq!(20, rows.get(9).unwrap().node.id);

    // Retrieve first 20 rows backwards
    let page = PageQuery::Backward(BackwardPageQuery {
        last: 20,
        before: Some(rows.first().unwrap().cursor.clone()),
    });

    let res = sqlx_query_paginated_as!(
        page, &pool,
        [timestamp.asc(): NaiveDateTime, id.desc(): i32],
        TodoRow,
        r#"SELECT * FROM "todo""#
    );

    assert!(!res.page_info.has_previous_page);

    let rows = res.into_iter().collect::<Vec<_>>();

    assert_eq!(10, rows.len());
    assert_eq!(1, rows.first().unwrap().node.id);
    assert_eq!(10, rows.get(9).unwrap().node.id);

    Ok(())
}

#[sqlx::test(migrator = "MIGRATIONS")]
async fn test_end(pool: PgPool) -> Result<()> {
    // tracing_subscriber::fmt().compact().with_env_filter("trace").init();

    // Retrieve the last 10 rows
    let page = PageQuery::Backward(BackwardPageQuery { last: 10, before: None });

    let res = sqlx_query_paginated_as!(
        page, &pool,
        [timestamp.asc(): NaiveDateTime, id.desc(): i32],
        TodoRow,
        r#"SELECT * FROM "todo" WHERE "id" != $1 AND "id" != $2"#,
        50i32,
        60i32
    );

    assert!(!res.page_info.has_next_page);
    assert!(res.page_info.has_previous_page);

    let rows = res.into_iter().collect::<Vec<_>>();

    assert_eq!(10, rows.len());
    assert_eq!(91, rows.first().unwrap().node.id);
    assert_eq!(100, rows.get(9).unwrap().node.id);

    // Retrieve previous 10 rows
    let page = PageQuery::Backward(BackwardPageQuery {
        last: 10,
        before: Some(rows.first().unwrap().cursor.clone()),
    });

    let res = sqlx_query_paginated_as!(
        page, &pool,
        [timestamp.asc(): NaiveDateTime, id.desc(): i32],
        TodoRow,
        r#"SELECT * FROM "todo""#
    );

    assert!(res.page_info.has_previous_page);

    let rows = res.into_iter().collect::<Vec<_>>();

    assert_eq!(10, rows.len());
    assert_eq!(81, rows.first().unwrap().node.id);
    assert_eq!(90, rows.get(9).unwrap().node.id);

    // Retrieve last 20 rows forwards
    let page = PageQuery::Forward(ForwardPageQuery {
        first: 20,
        after: Some(rows.last().unwrap().cursor.clone()),
    });

    let res = sqlx_query_paginated_as!(
        page, &pool,
        [timestamp.asc(): NaiveDateTime, id.desc(): i32],
        TodoRow,
        r#"SELECT * FROM "todo""#
    );

    assert!(!res.page_info.has_next_page);

    let rows = res.into_iter().collect::<Vec<_>>();

    assert_eq!(10, rows.len());
    assert_eq!(91, rows.first().unwrap().node.id);
    assert_eq!(100, rows.get(9).unwrap().node.id);

    Ok(())
}
