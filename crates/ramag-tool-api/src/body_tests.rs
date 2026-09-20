use super::*;

use gpui::TestAppContext;
use ramag_domain::entities::ApiRequestRecord;

#[gpui::test]
fn api_multipart_editor_builds_domain_request_body(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let Some(view) = view_entity else {
        return;
    };
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.http_body_mode = ApiBodyMode::Multipart;
            view.http_body.update(cx, |input, cx| {
                input.set_value(
                    "text|title|hello\nfile|upload|/tmp/data.bin|data.bin|application/octet-stream",
                    window,
                    cx,
                )
            });
        });
    });

    let request = visual_cx.update(|_, app| {
        let view = view.read(app);
        request_from_view(view, app)
    });
    assert!(request.is_ok(), "Multipart 编辑器应能构造请求：{request:?}");
    let Ok(ApiRequestSpec::Http(spec)) = request else {
        return;
    };
    let Some(body) = spec.body else {
        return;
    };
    assert_eq!(body.mode, ApiBodyMode::Multipart);
    assert_eq!(body.multipart.len(), 2);
}

#[gpui::test]
fn api_imported_multipart_body_populates_editor_without_losing_parts(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let Some(view) = view_entity else {
        return;
    };
    let mut request = HttpRequestSpec::new("POST", "http://127.0.0.1/upload");
    request.body = Some(ApiBody::multipart(vec![
        ApiMultipartPart::text("title", "hello", false),
        ApiMultipartPart::file(
            "upload",
            "/tmp/data.bin",
            Some("data.bin".into()),
            Some("application/octet-stream".into()),
        ),
    ]));
    let mut workspace = ApiWorkspace::new("Imported");
    let mut collection = ApiCollection::new("Uploads");
    collection
        .requests
        .push(ApiRequestRecord::new_http("Multipart upload", request));
    workspace.collections.push(collection);

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.workspace = workspace.clone();
            context::apply_imported_workspace(view, &workspace, window, cx);
        });
    });
    let state = visual_cx.update(|_, app| {
        let view = view.read(app);
        (
            view.http_body_mode,
            view.http_body.read(app).value().to_string(),
            view.http_body_content_type.clone(),
        )
    });
    assert_eq!(state.0, ApiBodyMode::Multipart);
    assert!(state.1.contains("text|title|hello"));
    assert!(
        state
            .1
            .contains("file|upload|/tmp/data.bin|data.bin|application/octet-stream")
    );
    assert!(state.2.is_empty());
}

#[test]
fn api_multipart_editor_format_parses_and_formats_text_and_file_parts()
-> std::result::Result<(), String> {
    let parts = parse_multipart_body(
        "text|title|hello|secret\nfile|upload|/tmp/data.bin|data.bin|application/octet-stream",
    )
    .map_err(|error| error.to_string())?;
    assert_eq!(parts.len(), 2);
    assert!(matches!(
        parts[0].value,
        ApiMultipartValue::Text { ref value } if value == "hello"
    ));
    assert!(parts[0].sensitive);
    assert!(matches!(
        parts[1].value,
        ApiMultipartValue::File { ref path, ref file_name }
            if path == "/tmp/data.bin" && file_name.as_deref() == Some("data.bin")
    ));
    assert_eq!(
        format_multipart_body(&parts),
        "text|title|hello|secret\nfile|upload|/tmp/data.bin|data.bin|application/octet-stream"
    );
    assert!(parse_multipart_body("binary|field|value").is_err());
    assert!(parse_multipart_body("file|field").is_err());
    Ok(())
}
