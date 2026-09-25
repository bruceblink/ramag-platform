use super::*;

impl TableDesigner {
    pub(super) fn change_sql(&self, cx: &gpui_kit::App) -> Result<String, String> {
        let qualified = format!(
            "{}.{}",
            self.driver.quote_identifier(&self.schema),
            self.driver.quote_identifier(&self.original_table)
        );
        let mut statements = Vec::new();
        let mut mysql_alter_clauses = Vec::new();
        let mut names = HashSet::new();
        for field in &self.fields {
            let name = field.name.read(cx).value().trim().to_string();
            let data_type = field.data_type.read(cx).value().trim().to_string();
            let default_value = field.default_value.read(cx).value().trim().to_string();
            let comment = field.comment.read(cx).value().trim().to_string();
            if field.deleted {
                if let Some(original) = &field.original {
                    let column = self.driver.quote_identifier(&original.name);
                    if self.driver == DriverKind::Mysql {
                        mysql_alter_clauses.push(format!("DROP COLUMN {column}"));
                    } else {
                        statements.push(format!("ALTER TABLE {qualified} DROP COLUMN {column};"));
                    }
                }
                continue;
            }
            validate_identifier("字段名", &name)?;
            if !names.insert(name.to_ascii_lowercase()) {
                return Err(format!("字段名 {name} 重复，请修改后再预览"));
            }
            if data_type.is_empty() {
                return Err(format!("字段 {name} 的类型不能为空"));
            }
            if data_type.contains(';') {
                return Err(format!("字段 {name} 的类型不能包含分号"));
            }
            if default_value.contains(';') {
                return Err(format!("字段 {name} 的默认值不能包含分号"));
            }
            let sql = FieldSql {
                name: &name,
                data_type: &data_type,
                default_value: &default_value,
                comment: &comment,
            };
            match self.driver {
                DriverKind::Mysql => self.mysql_field_sql(field, &sql, &mut mysql_alter_clauses),
                DriverKind::Postgres => {
                    self.postgres_field_sql(field, &qualified, &sql, &mut statements);
                    Ok(())
                }
                DriverKind::Sqlite => {
                    self.sqlite_field_sql(field, &qualified, &sql, &mut statements)
                }
                _ => return Err("当前数据库不支持表结构设计器".into()),
            }?;
        }
        if !mysql_alter_clauses.is_empty() {
            statements.push(format!(
                "ALTER TABLE {qualified} {};",
                mysql_alter_clauses.join(",\n    ")
            ));
        }
        if statements.is_empty() {
            Err(NO_CHANGES.into())
        } else {
            Ok(statements.join("\n"))
        }
    }

    pub(super) fn rename_sql(&self, cx: &gpui_kit::App) -> Result<String, String> {
        let table = self.table_name.read(cx).value().trim().to_string();
        validate_identifier("表名", &table)?;
        if self.original_table == table {
            return Err(NO_CHANGES.into());
        }
        let schema = self.driver.quote_identifier(&self.schema);
        let old = self.driver.quote_identifier(&self.original_table);
        let new = self.driver.quote_identifier(&table);
        match self.driver {
            DriverKind::Mysql => Ok(format!("RENAME TABLE {schema}.{old} TO {schema}.{new};")),
            DriverKind::Postgres => Ok(format!("ALTER TABLE {schema}.{old} RENAME TO {new};")),
            DriverKind::Sqlite => Ok(format!("ALTER TABLE {schema}.{old} RENAME TO {new};")),
            _ => Err("当前数据库不支持表结构设计器".into()),
        }
    }
    /// 生成 MySQL 字段变更并保留数据库返回的生成属性。
    ///
    /// `CHANGE COLUMN` 会重写整列定义；如果只输出编辑器中的类型、空值、默认值和注释，
    /// MySQL 会静默丢掉 `AUTO_INCREMENT` 或生成列表达式。因此旧字段的元数据必须随定义
    /// 一起重放；无法完整表达的元数据直接拒绝预览，避免执行后不可逆地改变列语义。
    fn mysql_field_sql(
        &self,
        field: &FieldDraft,
        sql: &FieldSql<'_>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        let definition = mysql_definition(
            self.driver,
            sql.name,
            sql.data_type,
            field.nullable,
            sql.default_value,
            sql.comment,
            field.original.as_ref(),
        )?;
        match &field.original {
            None => out.push(format!("ADD COLUMN {definition}")),
            Some(original)
                if field_changed(
                    field,
                    original,
                    sql.name,
                    sql.data_type,
                    sql.default_value,
                    sql.comment,
                ) =>
            {
                out.push(format!(
                    "CHANGE COLUMN {} {definition}",
                    self.driver.quote_identifier(&original.name)
                ))
            }
            _ => {}
        }
        Ok(())
    }

    fn postgres_field_sql(
        &self,
        field: &FieldDraft,
        table: &str,
        sql: &FieldSql<'_>,
        out: &mut Vec<String>,
    ) {
        let qname = self.driver.quote_identifier(sql.name);
        let Some(original) = &field.original else {
            let null = if field.nullable { "" } else { " NOT NULL" };
            let default = if sql.default_value.is_empty() {
                String::new()
            } else {
                format!(" DEFAULT {}", sql.default_value)
            };
            out.push(format!(
                "ALTER TABLE {table} ADD COLUMN {qname} {}{null}{default};",
                sql.data_type
            ));
            if !sql.comment.is_empty() {
                out.push(format!(
                    "COMMENT ON COLUMN {table}.{qname} IS '{}';",
                    escape_literal(sql.comment)
                ));
            }
            return;
        };
        if original.name != sql.name {
            out.push(format!(
                "ALTER TABLE {table} RENAME COLUMN {} TO {qname};",
                self.driver.quote_identifier(&original.name)
            ));
        }
        if original.data_type.raw_type != sql.data_type {
            out.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {qname} TYPE {};",
                sql.data_type
            ));
        }
        if original.nullable != field.nullable {
            out.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {qname} {};",
                if field.nullable {
                    "DROP NOT NULL"
                } else {
                    "SET NOT NULL"
                }
            ));
        }
        if original.default_value.as_deref().unwrap_or("") != sql.default_value {
            out.push(if sql.default_value.is_empty() {
                format!("ALTER TABLE {table} ALTER COLUMN {qname} DROP DEFAULT;")
            } else {
                format!(
                    "ALTER TABLE {table} ALTER COLUMN {qname} SET DEFAULT {};",
                    sql.default_value
                )
            });
        }
        if original.comment.as_deref().unwrap_or("") != sql.comment {
            out.push(format!(
                "COMMENT ON COLUMN {table}.{qname} IS {};",
                if sql.comment.is_empty() {
                    "NULL".into()
                } else {
                    format!("'{}'", escape_literal(sql.comment))
                }
            ));
        }
    }

    /// 生成 SQLite 能直接执行的字段变更；类型和约束变化需要重建表，不能伪装成 ALTER COLUMN。
    fn sqlite_field_sql(
        &self,
        field: &FieldDraft,
        table: &str,
        sql: &FieldSql<'_>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        if !sql.comment.is_empty() {
            return Err("SQLite 不支持字段注释，请在 DDL 或查询编辑器中维护注释信息".into());
        }
        let qname = self.driver.quote_identifier(sql.name);
        let null = if field.nullable { " NULL" } else { " NOT NULL" };
        let default = if sql.default_value.is_empty() {
            String::new()
        } else {
            format!(" DEFAULT {}", sql.default_value)
        };
        let Some(original) = &field.original else {
            out.push(format!(
                "ALTER TABLE {table} ADD COLUMN {qname} {}{null}{default};",
                sql.data_type
            ));
            return Ok(());
        };

        let definition_changed = original.data_type.raw_type != sql.data_type
            || original.nullable != field.nullable
            || original.default_value.as_deref().unwrap_or("") != sql.default_value
            || original.comment.as_deref().unwrap_or("") != sql.comment
            || original.is_primary_key
            || original.is_auto_increment
            || original.generation_expression.is_some()
            || original.generated_storage.is_some()
            || original.identity_generation.is_some();
        if definition_changed {
            return Err(format!(
                "SQLite 字段 {} 只支持重命名；类型、约束或默认值变化请使用重建表 SQL",
                original.name
            ));
        }
        if original.name != sql.name {
            out.push(format!(
                "ALTER TABLE {table} RENAME COLUMN {} TO {qname};",
                self.driver.quote_identifier(&original.name)
            ));
        }
        Ok(())
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label}不能为空"));
    }
    if value.len() > MAX_CONNECTION_IDENTIFIER_BYTES {
        return Err(format!(
            "{label}不能超过 {MAX_CONNECTION_IDENTIFIER_BYTES} 字节"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{label}不能包含控制字符"));
    }
    Ok(())
}
fn mysql_definition(
    driver: DriverKind,
    name: &str,
    data_type: &str,
    nullable: bool,
    default_value: &str,
    comment: &str,
    original: Option<&Column>,
) -> Result<String, String> {
    let attributes = mysql_attributes(original)?;
    if attributes.generated && !default_value.trim().is_empty() {
        return Err(format!(
            "MySQL 字段 {name} 同时包含生成表达式和默认值，无法安全生成 CHANGE COLUMN"
        ));
    }
    if attributes.generated && attributes.auto_increment {
        return Err(format!(
            "MySQL 字段 {name} 同时包含生成表达式和 AUTO_INCREMENT，无法安全生成 CHANGE COLUMN"
        ));
    }
    let null = if nullable { " NULL" } else { " NOT NULL" };
    let default = if attributes.generated || default_value.is_empty() {
        String::new()
    } else {
        format!(" DEFAULT {default_value}")
    };
    let comment = if comment.is_empty() {
        String::new()
    } else {
        format!(" COMMENT '{}'", escape_literal(comment))
    };
    let generated = attributes.generated_clause;
    let auto_increment = if attributes.auto_increment {
        " AUTO_INCREMENT"
    } else {
        ""
    };
    Ok(format!(
        "{} {data_type}{generated}{null}{default}{auto_increment}{comment}",
        driver.quote_identifier(name)
    ))
}

/// MySQL `CHANGE COLUMN` 需要完整重放的列级属性。
///
/// 这些值只来自已加载的列元数据，不是编辑器中的自由输入；生成列表达式和自增标记
/// 必须与原列一起输出，否则一次只改注释或类型也可能改变后续插入行为。
struct MysqlAttributes {
    generated_clause: String,
    generated: bool,
    auto_increment: bool,
}

/// 将 MySQL 原字段元数据转换为可重放的定义片段，并拒绝跨方言属性。
fn mysql_attributes(original: Option<&Column>) -> Result<MysqlAttributes, String> {
    let Some(original) = original else {
        return Ok(MysqlAttributes {
            generated_clause: String::new(),
            generated: false,
            auto_increment: false,
        });
    };
    if original.identity_generation.is_some() {
        return Err(format!(
            "MySQL 字段 {} 包含不支持的 IDENTITY 属性，无法安全生成 CHANGE COLUMN",
            original.name
        ));
    }
    let expression = original
        .generation_expression
        .as_deref()
        .map(str::trim)
        .filter(|expression| !expression.is_empty());
    if expression.is_some() != original.generated_storage.is_some() {
        return Err(format!(
            "MySQL 字段 {} 的生成列元数据不完整，无法安全生成 CHANGE COLUMN",
            original.name
        ));
    }
    let generated_clause = match (expression, original.generated_storage) {
        (None, None) => String::new(),
        (Some(expression), Some(storage)) => {
            let storage = match storage {
                ramag_domain::entities::GeneratedColumnStorage::Virtual => "VIRTUAL",
                ramag_domain::entities::GeneratedColumnStorage::Stored => "STORED",
            };
            format!(" GENERATED ALWAYS AS ({expression}) {storage}")
        }
        _ => unreachable!("生成列元数据完整性已在上方检查"),
    };
    Ok(MysqlAttributes {
        generated: expression.is_some(),
        generated_clause,
        auto_increment: original.is_auto_increment,
    })
}

fn escape_literal(value: &str) -> String {
    value.replace('\'', "''")
}
