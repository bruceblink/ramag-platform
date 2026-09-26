use super::*;
use ramag_domain::entities::{Column, ColumnKind, ColumnType, DriverKind};

fn column(name: &str, raw_type: &str) -> Column {
    Column {
        name: name.into(),
        data_type: ColumnType {
            kind: ColumnKind::Other,
            raw_type: raw_type.into(),
        },
        nullable: true,
        default_value: None,
        is_primary_key: false,
        comment: None,
        is_auto_increment: false,
        generation_expression: None,
        generated_storage: None,
        identity_generation: None,
        ordinal_position: None,
    }
}

#[test]
fn mysql_unique_rename_candidate_preserves_the_existing_column() {
    let mut source_column = column("email", "VARCHAR(255)");
    source_column.ordinal_position = Some(1);
    let mut target_column = column("legacy_email", "VARCHAR(255)");
    target_column.ordinal_position = Some(1);
    let source = TableMetadata {
        columns: vec![source_column],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        columns: vec![target_column],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Mysql,
        "app",
        "users",
        "app",
        "users",
        &source,
        &target,
    )
    .expect("migration script");

    assert!(
        script
            .sql
            .contains("CHANGE COLUMN `legacy_email` `email` VARCHAR(255) NULL")
    );
    assert!(!script.sql.contains("DROP COLUMN `legacy_email`"));
    assert!(!script.sql.contains("ADD COLUMN `email`"));
    assert_eq!(script.statement_count, 1);
}

#[test]
fn postgres_unique_rename_candidate_uses_rename_column() {
    let mut source_column = column("email", "text");
    source_column.ordinal_position = Some(1);
    let mut target_column = column("legacy_email", "text");
    target_column.ordinal_position = Some(1);
    let source = TableMetadata {
        columns: vec![source_column],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        columns: vec![target_column],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Postgres,
        "app",
        "users",
        "app",
        "users",
        &source,
        &target,
    )
    .expect("migration script");

    assert!(
        script
            .sql
            .contains("RENAME COLUMN \"legacy_email\" TO \"email\"")
    );
    assert!(!script.sql.contains("DROP COLUMN"));
    assert!(!script.sql.contains("ADD COLUMN"));
    assert_eq!(script.statement_count, 1);
}

#[test]
fn sqlite_unique_rename_candidate_uses_rename_column() {
    let mut source_column = column("email", "TEXT");
    source_column.ordinal_position = Some(1);
    let mut target_column = column("legacy_email", "TEXT");
    target_column.ordinal_position = Some(1);
    let source = TableMetadata {
        columns: vec![source_column],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        columns: vec![target_column],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Sqlite,
        "main",
        "users",
        "main",
        "users",
        &source,
        &target,
    )
    .expect("migration script");

    assert!(
        script
            .sql
            .contains("RENAME COLUMN \"legacy_email\" TO \"email\"")
    );
    assert!(!script.sql.contains("DROP COLUMN"));
    assert!(!script.sql.contains("ADD COLUMN"));
    assert_eq!(script.statement_count, 1);
}

#[test]
fn ambiguous_rename_candidates_stay_as_drop_and_add() {
    let source = TableMetadata {
        columns: vec![column("email", "TEXT"), column("contact", "TEXT")],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        columns: vec![column("legacy_email", "TEXT")],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Postgres,
        "app",
        "users",
        "app",
        "users",
        &source,
        &target,
    )
    .expect("migration script");

    assert!(script.sql.contains("DROP COLUMN \"legacy_email\""));
    assert!(script.sql.contains("ADD COLUMN \"email\""));
    assert!(script.sql.contains("ADD COLUMN \"contact\""));
    assert!(!script.sql.contains("RENAME COLUMN"));
}

#[test]
fn changed_column_definition_does_not_become_a_rename() {
    let source = TableMetadata {
        columns: vec![column("email", "VARCHAR(320)")],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        columns: vec![column("legacy_email", "VARCHAR(255)")],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Postgres,
        "app",
        "users",
        "app",
        "users",
        &source,
        &target,
    )
    .expect("migration script");

    assert!(script.sql.contains("DROP COLUMN \"legacy_email\""));
    assert!(script.sql.contains("ADD COLUMN \"email\""));
    assert!(!script.sql.contains("RENAME COLUMN"));
}

#[test]
fn incomplete_position_metadata_does_not_become_a_rename() {
    let source = TableMetadata {
        columns: vec![column("email", "TEXT")],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        columns: vec![column("legacy_email", "TEXT")],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Postgres,
        "app",
        "users",
        "app",
        "users",
        &source,
        &target,
    )
    .expect("migration script");

    assert!(script.sql.contains("DROP COLUMN \"legacy_email\""));
    assert!(script.sql.contains("ADD COLUMN \"email\""));
    assert!(!script.sql.contains("RENAME COLUMN"));
}
