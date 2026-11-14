use super::*;

#[test]
fn template_expansion_uses_the_domain_byte_budget_and_error_mapping() {
    let variables = BTreeMap::from([("v".into(), "雪".into())]);
    for (template, budget, expected) in [
        ("{{v}}{{v}}", 6, "雪雪"),
        ("a{{v}}z", 5, "a雪z"),
        ("literal", 7, "literal"),
        ("{{v}}", 3, "雪"),
        ("", 0, ""),
    ] {
        let expanded = super::super::expand_template(template, &variables, "测试", budget);
        assert_eq!(expanded.ok().as_deref(), Some(expected));
    }
    for (template, budget) in [
        ("{{v}}{{v}}", 5),
        ("a{{v}}z", 4),
        ("literal", 6),
        ("{{v}}", 2),
        ("x", 0),
    ] {
        assert!(matches!(
            super::super::expand_template(template, &variables, "测试", budget),
            Err(DomainError::InvalidConfig(message))
                if message == format!("测试展开后超过 {budget} bytes 上限")
        ));
    }
    for (template, expected) in [
        ("{{missing}}", "缺少 API 环境变量：missing"),
        ("{{v", "测试变量占位符未闭合"),
        ("{{ }}", "测试变量名无效"),
    ] {
        assert!(matches!(
            super::super::expand_template(template, &variables, "测试", 32),
            Err(DomainError::InvalidConfig(message)) if message == expected
        ));
    }
}

#[tokio::test]
async fn oversized_expanded_url_is_rejected_before_any_http_request() {
    use ramag_domain::entities::MAX_API_URL_TEMPLATE_BYTES;

    let mut server = spawn_server(response("200 OK", &[], b"unexpected"), None).await;
    let request = ApiRequestSpec::Http(HttpRequestSpec::new(
        "GET",
        format!("{}/{{{{payload}}}}{{{{payload}}}}", server.url("")),
    ));
    let variables =
        BTreeMap::from([("payload".into(), "x".repeat(MAX_API_URL_TEMPLATE_BYTES / 2))]);
    let driver = HttpApiDriver::new().expect("create real HTTP driver");
    let result = driver
        .execute(&request, &variables, Arc::new(AtomicBool::new(false)))
        .await;
    let received = tokio::time::timeout(Duration::from_millis(50), server.wait_for_request()).await;
    server.stop().await;

    assert!(matches!(
        result,
        Err(DomainError::InvalidConfig(message))
            if message == format!("HTTP URL 模板展开后超过 {MAX_API_URL_TEMPLATE_BYTES} bytes 上限")
    ));
    assert!(received.is_err(), "an oversized URL must not be sent");
}
