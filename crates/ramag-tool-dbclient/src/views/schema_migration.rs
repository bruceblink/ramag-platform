//! 根据两张 SQL 表的元数据生成只读迁移脚本。

use ramag_domain::entities::DriverKind;

use super::schema_diff::TableMetadata;

mod generator;
pub(crate) use generator::MigrationStage;

#[cfg(test)]
mod sqlite_tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MigrationScript {
    pub(crate) sql: String,
    pub(crate) warnings: Vec<String>,
    pub(crate) statement_count: usize,
    pub(crate) destructive_statements: usize,
    pub(crate) stages: Vec<MigrationStage>,
}

/// 生成将目标表调整为源表结构的 SQL；此函数只读取元数据，不执行 SQL。
pub(crate) fn build_migration_script(
    driver: DriverKind,
    source_schema: &str,
    source_table: &str,
    target_schema: &str,
    target_table: &str,
    source: &TableMetadata,
    target: &TableMetadata,
) -> Result<MigrationScript, String> {
    if !matches!(
        driver,
        DriverKind::Mysql | DriverKind::Postgres | DriverKind::Sqlite
    ) {
        return Err("当前数据库不支持生成表结构迁移 SQL".into());
    }
    let source_name = generator::qualified_name(driver, source_schema, source_table)?;
    let target_name = generator::qualified_name(driver, target_schema, target_table)?;
    let mut statements = Vec::new();
    let mut warnings = Vec::new();

    // 先删除会阻止列变更的外键和索引，再处理列，最后恢复新增对象。
    generator::append_foreign_key_drops(
        driver,
        &target_name,
        &source.columns,
        &target.columns,
        &source.foreign_keys,
        &target.foreign_keys,
        &mut statements,
    )?;
    generator::append_index_drops(
        driver,
        &target_name,
        target_schema,
        &source.columns,
        &target.columns,
        &source.indexes,
        &target.indexes,
        &mut statements,
    )?;
    generator::append_column_changes(
        driver,
        &target_name,
        &source.columns,
        &target.columns,
        &mut statements,
    )?;
    generator::append_index_additions(
        driver,
        &target_name,
        &source.columns,
        &target.columns,
        &source.indexes,
        &target.indexes,
        &mut statements,
    )?;
    generator::append_foreign_key_additions(
        driver,
        &target_name,
        &source.foreign_keys,
        &target.foreign_keys,
        &mut statements,
    )?;

    let has_column_changes = generator::has_column_changes(&source.columns, &target.columns);
    if has_column_changes
        && generator::has_incomplete_column_metadata(&source.columns, &target.columns)
    {
        warnings.push(
            "列顺序元数据不完整（缺少数据库列序号），迁移不会根据缺失信息推断，执行前请人工复核"
                .into(),
        );
    }
    if driver == DriverKind::Postgres
        && generator::has_column_order_changes(&source.columns, &target.columns)
    {
        warnings
            .push("PostgreSQL 不支持直接调整现有列顺序，脚本不会自动重排，执行前请人工复核".into());
    }
    if driver == DriverKind::Postgres
        && generator::has_generated_column_rebuilds(&source.columns, &target.columns)
    {
        warnings.push(
            "生成列表达式变化会删除并重建目标列，可能丢失该列现有值，执行前请人工复核".into(),
        );
    } else if generator::has_generated_column_changes(&source.columns, &target.columns) {
        warnings.push("生成列属性变化会写入迁移 SQL，执行前请人工复核".into());
    }
    if generator::has_auto_generation_changes(&source.columns, &target.columns) {
        warnings.push(
            "自增或 IDENTITY 属性发生变化，迁移会显式写入新的生成规则，请确认序列行为".into(),
        );
    }
    if source
        .indexes
        .iter()
        .any(|index| index.unique && !index.primary)
    {
        warnings
            .push("唯一索引与唯一约束在当前元数据中无法区分，PostgreSQL 将按唯一索引生成".into());
    }
    if driver == DriverKind::Mysql && !statements.is_empty() {
        warnings
            .push("MySQL DDL 可能隐式提交；执行失败时目标表可能已部分变更，请先人工复核".into());
    }

    let destructive_statements = statements
        .iter()
        .filter(|statement| statement.destructive)
        .count();
    let sql = generator::format_script(
        driver,
        &source_name,
        &target_name,
        statements.iter().map(|statement| statement.sql.as_str()),
    );
    Ok(MigrationScript {
        statement_count: statements.len(),
        destructive_statements,
        stages: generator::summarize_stages(&statements),
        sql,
        warnings,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "schema_migration/rename_tests.rs"]
mod rename_tests;
