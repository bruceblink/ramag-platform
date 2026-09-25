use super::*;

fn required_new_field_sql(
    table_has_rows: Option<bool>,
    default_value: Option<&str>,
    cx: &mut TestAppContext,
) -> Result<String, String> {
    let (designer, cx) =
        designer_with_table_state(DriverKind::Sqlite, Vec::new(), table_has_rows, cx);
    cx.update(|window, app| {
        designer.update(app, |designer, cx| {
            designer.add_field(window, cx);
            designer.fields[0].nullable = false;
            if let Some(default_value) = default_value {
                designer.fields[0]
                    .default_value
                    .update(cx, |input, cx| input.set_value(default_value, window, cx));
            }
        });
    });
    cx.update(|_, app| designer.read(app).change_sql(app))
}

#[gpui_kit::test]
fn sqlite_empty_table_allows_required_new_column(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);

    let sql =
        required_new_field_sql(Some(false), None, cx).expect("空 SQLite 表应允许新增必填字段");

    assert_eq!(
        sql,
        "ALTER TABLE \"public\".\"users\" ADD COLUMN \"new_column\" VARCHAR(255) NOT NULL;"
    );
}

#[gpui_kit::test]
fn sqlite_non_empty_table_rejects_required_new_column_without_default(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);

    let error = required_new_field_sql(Some(true), None, cx)
        .expect_err("非空 SQLite 表不应生成无默认值的必填字段");

    assert!(error.contains("已有数据"));
    assert!(error.contains("默认值或允许 NULL"));
}

#[gpui_kit::test]
fn sqlite_non_empty_table_allows_required_new_column_with_default(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);

    let sql = required_new_field_sql(Some(true), Some("0"), cx)
        .expect("非空 SQLite 表有默认值时应允许新增必填字段");

    assert!(sql.contains("ADD COLUMN \"new_column\" VARCHAR(255) NOT NULL DEFAULT 0;"));
}

#[gpui_kit::test]
fn sqlite_unknown_table_state_rejects_required_new_column(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);

    let error = required_new_field_sql(None, None, cx)
        .expect_err("未知 SQLite 表状态不应生成危险的必填字段");

    assert!(error.contains("无法确认 SQLite 表 users 是否为空"));
}
