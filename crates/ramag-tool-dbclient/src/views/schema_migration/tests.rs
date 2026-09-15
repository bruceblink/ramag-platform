use super::*;
use ramag_domain::entities::{
    Column, ColumnKind, ColumnType, ForeignKey, ForeignKeyAction, GeneratedColumnStorage,
    IdentityGeneration, Index,
};

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
        ordinal_position: None,
        is_auto_increment: false,
        generation_expression: None,
        generated_storage: None,
        identity_generation: None,
    }
}

fn index(name: &str, columns: &[&str]) -> Index {
    Index {
        name: name.into(),
        unique: false,
        primary: false,
        columns: columns.iter().map(|column| (*column).into()).collect(),
    }
}

#[test]
fn mysql_migration_drops_dependencies_before_columns_and_restores_them() {
    let source = TableMetadata {
        columns: vec![column("id", "INT"), column("email", "VARCHAR(255)")],
        indexes: vec![Index {
            primary: true,
            name: "PRIMARY".into(),
            ..index("PRIMARY", &["id"])
        }],
        foreign_keys: vec![],
    };
    let target = TableMetadata {
        columns: vec![column("id", "BIGINT"), column("legacy", "TEXT")],
        indexes: vec![Index {
            primary: true,
            name: "PRIMARY".into(),
            ..index("PRIMARY", &["id"])
        }],
        foreign_keys: vec![],
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
    let drop_index = script
        .sql
        .find("DROP PRIMARY KEY")
        .expect("drop primary key");
    let drop_column = script
        .sql
        .find("DROP COLUMN `legacy`")
        .expect("drop column");
    let add_column = script.sql.find("ADD COLUMN `email`").expect("add column");
    let add_index = script
        .sql
        .rfind("ADD PRIMARY KEY")
        .expect("add primary key");
    assert!(drop_index < drop_column);
    assert!(drop_column < add_column);
    assert!(add_column < add_index);
    assert_eq!(script.destructive_statements, 3);
    assert_eq!(
        script
            .stages
            .iter()
            .map(|stage| stage.title)
            .collect::<Vec<_>>(),
        ["删除受影响的索引", "处理字段变化", "恢复索引"]
    );
    assert_eq!(script.stages[0].destructive_statements, 1);
    assert_eq!(script.stages[1].statement_count, 3);
    assert_eq!(script.stages[1].destructive_statements, 2);
    assert_eq!(script.stages[2].destructive_statements, 0);
    assert!(
        script
            .warnings
            .iter()
            .any(|warning| warning.contains("列顺序"))
    );
}

#[test]
fn postgres_migration_quotes_names_and_emits_comment_changes() {
    let mut old = column("UserName", "text");
    old.comment = Some("old".into());
    let mut new = column("username", "text");
    new.comment = Some("owner's email".into());
    let source = TableMetadata {
        columns: vec![new],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        columns: vec![old],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Postgres,
        "sales data",
        "users",
        "sales data",
        "users",
        &source,
        &target,
    )
    .expect("migration script");
    assert!(script.sql.contains(
        "ALTER TABLE \"sales data\".\"users\" RENAME COLUMN \"UserName\" TO \"username\";"
    ));
    assert!(script.sql.contains("IS 'owner''s email';"));
}

#[test]
fn unchanged_metadata_reports_no_statements() {
    let source = TableMetadata {
        columns: vec![column("id", "int")],
        ..TableMetadata::default()
    };
    let script = build_migration_script(
        DriverKind::Mysql,
        "app",
        "users",
        "app",
        "users",
        &source,
        &source,
    )
    .expect("migration script");
    assert_eq!(script.statement_count, 0);
    assert!(script.sql.contains("No schema changes detected"));
}

#[test]
fn unsafe_type_is_rejected_before_script_generation() {
    let source = TableMetadata {
        columns: vec![column("id", "INT; DROP TABLE users")],
        ..TableMetadata::default()
    };
    let error = build_migration_script(
        DriverKind::Mysql,
        "app",
        "users",
        "app",
        "users",
        &source,
        &TableMetadata::default(),
    )
    .expect_err("unsafe type should be rejected");
    assert!(error.contains("字段类型"));
}

#[test]
fn migration_preserves_foreign_key_actions() {
    let source = TableMetadata {
        foreign_keys: vec![ForeignKey {
            name: "fk_project".into(),
            columns: vec!["project_id".into()],
            ref_schema: "app".into(),
            ref_table: "projects".into(),
            ref_columns: vec!["id".into()],
            on_delete: ForeignKeyAction::Cascade,
            on_update: ForeignKeyAction::SetNull,
        }],
        ..TableMetadata::default()
    };
    let target = TableMetadata {
        foreign_keys: vec![ForeignKey {
            on_delete: ForeignKeyAction::NoAction,
            on_update: ForeignKeyAction::NoAction,
            ..source.foreign_keys[0].clone()
        }],
        ..TableMetadata::default()
    };

    let script = build_migration_script(
        DriverKind::Postgres,
        "app",
        "projects",
        "app",
        "projects_copy",
        &source,
        &target,
    )
    .expect("migration script");
    assert!(script.sql.contains("ON DELETE CASCADE ON UPDATE SET NULL"));
    assert!(
        !script
            .warnings
            .iter()
            .any(|warning| warning.contains("外键动作"))
    );
}

#[test]
fn mysql_migration_preserves_column_order_auto_increment_and_generated_columns() {
    let mut source_id = column("id", "INT");
    source_id.ordinal_position = Some(1);
    source_id.is_auto_increment = true;
    let mut source_total = column("total", "INT");
    source_total.ordinal_position = Some(2);
    source_total.generation_expression = Some("price + 1".into());
    source_total.generated_storage = Some(GeneratedColumnStorage::Stored);
    let mut source_price = column("price", "INT");
    source_price.ordinal_position = Some(3);

    let mut target_id = column("id", "INT");
    target_id.ordinal_position = Some(1);
    let mut target_price = column("price", "INT");
    target_price.ordinal_position = Some(2);
    let script = build_migration_script(
        DriverKind::Mysql,
        "app",
        "orders",
        "app",
        "orders",
        &TableMetadata {
            columns: vec![source_id, source_total, source_price],
            ..TableMetadata::default()
        },
        &TableMetadata {
            columns: vec![target_id, target_price],
            ..TableMetadata::default()
        },
    )
    .expect("migration script");

    assert!(
        script
            .sql
            .contains("CHANGE COLUMN `id` `id` INT NULL AUTO_INCREMENT FIRST;")
    );
    assert!(script.sql.contains(
        "ADD COLUMN `total` INT GENERATED ALWAYS AS (price + 1) STORED NULL AFTER `id`;"
    ));
    assert!(
        script
            .sql
            .contains("CHANGE COLUMN `price` `price` INT NULL AFTER `total`;")
    );
    assert!(
        script
            .warnings
            .iter()
            .any(|warning| warning.contains("自增"))
    );
}

#[test]
fn postgres_migration_emits_identity_and_rebuilds_generated_column() {
    let mut source_id = column("id", "bigint");
    source_id.ordinal_position = Some(1);
    source_id.is_auto_increment = true;
    source_id.identity_generation = Some(IdentityGeneration::Always);
    let mut source_total = column("total", "integer");
    source_total.ordinal_position = Some(2);
    source_total.generation_expression = Some("price * 2".into());
    source_total.generated_storage = Some(GeneratedColumnStorage::Stored);

    let mut target_id = column("id", "bigint");
    target_id.ordinal_position = Some(1);
    let mut target_total = column("total", "integer");
    target_total.ordinal_position = Some(2);
    let script = build_migration_script(
        DriverKind::Postgres,
        "app",
        "orders",
        "app",
        "orders",
        &TableMetadata {
            columns: vec![source_id, source_total],
            ..TableMetadata::default()
        },
        &TableMetadata {
            columns: vec![target_id, target_total],
            ..TableMetadata::default()
        },
    )
    .expect("migration script");

    assert!(script.sql.contains(
        "ALTER TABLE \"app\".\"orders\" ALTER COLUMN \"id\" ADD GENERATED ALWAYS AS IDENTITY;"
    ));
    assert!(
        script
            .sql
            .contains("ALTER TABLE \"app\".\"orders\" DROP COLUMN \"total\";")
    );
    assert!(script.sql.contains(
        "ALTER TABLE \"app\".\"orders\" ADD COLUMN \"total\" integer GENERATED ALWAYS AS (price * 2) STORED NULL;"
    ));
    assert!(
        script
            .warnings
            .iter()
            .any(|warning| warning.contains("重建目标列"))
    );
}

#[test]
fn mysql_generated_column_change_uses_change_column_warning() {
    let mut source_total = column("total", "integer");
    source_total.generation_expression = Some("price + 2".into());
    source_total.generated_storage = Some(GeneratedColumnStorage::Stored);
    let mut target_total = column("total", "integer");
    target_total.generation_expression = Some("price + 1".into());
    target_total.generated_storage = Some(GeneratedColumnStorage::Stored);
    let script = build_migration_script(
        DriverKind::Mysql,
        "app",
        "orders",
        "app",
        "orders",
        &TableMetadata {
            columns: vec![source_total],
            ..TableMetadata::default()
        },
        &TableMetadata {
            columns: vec![target_total],
            ..TableMetadata::default()
        },
    )
    .expect("migration script");

    assert!(script.sql.contains("CHANGE COLUMN `total`"));
    assert!(
        script
            .warnings
            .iter()
            .any(|warning| warning.contains("生成列属性变化"))
    );
    assert!(
        script
            .warnings
            .iter()
            .all(|warning| !warning.contains("删除并重建"))
    );
}

#[test]
fn postgres_column_order_change_is_reported_without_fake_reordering_sql() {
    let mut source_id = column("id", "integer");
    source_id.ordinal_position = Some(1);
    let mut source_name = column("name", "text");
    source_name.ordinal_position = Some(2);
    let mut target_id = column("id", "integer");
    target_id.ordinal_position = Some(2);
    let mut target_name = column("name", "text");
    target_name.ordinal_position = Some(1);
    let script = build_migration_script(
        DriverKind::Postgres,
        "app",
        "users",
        "app",
        "users",
        &TableMetadata {
            columns: vec![source_id, source_name],
            ..TableMetadata::default()
        },
        &TableMetadata {
            columns: vec![target_name, target_id],
            ..TableMetadata::default()
        },
    )
    .expect("migration script");

    assert_eq!(script.statement_count, 0);
    assert!(
        script
            .warnings
            .iter()
            .any(|warning| warning.contains("不会自动重排"))
    );
}

#[test]
fn column_order_change_does_not_rebuild_indexes_or_foreign_keys() {
    let mut source_id = column("id", "integer");
    source_id.ordinal_position = Some(1);
    let mut source_name = column("name", "text");
    source_name.ordinal_position = Some(2);
    let mut target_id = column("id", "integer");
    target_id.ordinal_position = Some(2);
    let mut target_name = column("name", "text");
    target_name.ordinal_position = Some(1);
    let source = TableMetadata {
        columns: vec![source_id, source_name],
        indexes: vec![Index {
            name: "idx_name".into(),
            columns: vec!["name".into()],
            ..index("idx_name", &[])
        }],
        foreign_keys: vec![],
    };
    let target = TableMetadata {
        columns: vec![target_name, target_id],
        indexes: source.indexes.clone(),
        foreign_keys: vec![],
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

    assert_eq!(script.statement_count, 0);
    assert!(
        script
            .warnings
            .iter()
            .any(|warning| warning.contains("不会自动重排"))
    );
    assert!(!script.sql.contains("DROP INDEX"));
    assert!(!script.sql.contains("CREATE INDEX"));
}

#[test]
fn plain_added_column_does_not_report_generation_attribute_changes() {
    let script = build_migration_script(
        DriverKind::Mysql,
        "app",
        "users",
        "app",
        "users",
        &TableMetadata {
            columns: vec![column("name", "VARCHAR(64)")],
            ..TableMetadata::default()
        },
        &TableMetadata::default(),
    )
    .expect("migration script");

    assert!(
        script
            .warnings
            .iter()
            .all(|warning| !warning.contains("自动生成") && !warning.contains("自增"))
    );
}
