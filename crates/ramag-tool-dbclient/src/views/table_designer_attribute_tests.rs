use super::*;
use ramag_domain::entities::{GeneratedColumnStorage, IdentityGeneration};

#[gpui_kit::test]
fn mysql_change_preserves_auto_increment_and_generated_column_attributes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut id = column("id", "INT", false);
    id.is_primary_key = true;
    id.is_auto_increment = true;
    let mut total = column("total", "INT", true);
    total.generation_expression = Some("price + 1".into());
    total.generated_storage = Some(GeneratedColumnStorage::Stored);
    let (designer, cx) = designer(DriverKind::Mysql, vec![id, total], cx);
    cx.update(|window, app| {
        designer.update(app, |designer, cx| {
            designer.fields[0]
                .data_type
                .update(cx, |input, cx| input.set_value("BIGINT", window, cx));
            designer.fields[0]
                .comment
                .update(cx, |input, cx| input.set_value("identifier", window, cx));
            designer.fields[1]
                .data_type
                .update(cx, |input, cx| input.set_value("BIGINT", window, cx));
            designer.fields[1].nullable = false;
            designer.fields[1].comment.update(cx, |input, cx| {
                input.set_value("computed total", window, cx)
            });
        });
    });

    let sql = cx
        .update(|_, app| designer.read(app).change_sql(app))
        .expect("修改字段注释应生成 MySQL SQL");

    assert!(
        sql.contains("CHANGE COLUMN `id` `id` BIGINT NOT NULL AUTO_INCREMENT COMMENT 'identifier'")
    );
    assert!(sql.contains(
        "CHANGE COLUMN `total` `total` BIGINT GENERATED ALWAYS AS (price + 1) STORED NOT NULL COMMENT 'computed total'"
    ));
}

#[gpui_kit::test]
fn mysql_change_rejects_incomplete_generated_metadata(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut generated = column("total", "INT", true);
    generated.generation_expression = Some("price + 1".into());
    let (designer, cx) = designer(DriverKind::Mysql, vec![generated], cx);
    cx.update(|window, app| {
        designer.update(app, |designer, cx| {
            designer.fields[0].comment.update(cx, |input, cx| {
                input.set_value("computed total", window, cx)
            });
        });
    });

    let error = cx
        .update(|_, app| designer.read(app).change_sql(app))
        .expect_err("生成列元数据不完整时必须拒绝 SQL");
    assert!(error.contains("生成列元数据不完整"));
}

#[gpui_kit::test]
fn mysql_change_rejects_identity_metadata(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut identity = column("id", "INT", false);
    identity.identity_generation = Some(IdentityGeneration::Always);
    let (designer, cx) = designer(DriverKind::Mysql, vec![identity], cx);
    cx.update(|window, app| {
        designer.update(app, |designer, cx| {
            designer.fields[0]
                .comment
                .update(cx, |input, cx| input.set_value("identifier", window, cx));
        });
    });

    let error = cx
        .update(|_, app| designer.read(app).change_sql(app))
        .expect_err("MySQL 不支持 IDENTITY 时必须拒绝 SQL");
    assert!(error.contains("不支持的 IDENTITY 属性"));
}
