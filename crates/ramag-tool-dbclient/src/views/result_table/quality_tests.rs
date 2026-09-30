use super::*;
use ramag_domain::entities::{Row, Value};

fn sample_result() -> QueryResult {
    QueryResult {
        columns: vec!["id".into(), "name".into()],
        column_types: vec!["BIGINT".into(), "TEXT".into()],
        rows: vec![
            Row {
                values: vec![Value::Int(2), Value::Text("Beta".into())],
            },
            Row {
                values: vec![Value::Int(1), Value::Text("alpha".into())],
            },
            Row {
                values: vec![Value::Int(3), Value::Text("Gamma".into())],
            },
        ],
        affected_rows: 0,
        elapsed_ms: 1,
        warnings: Vec::new(),
        truncated: false,
    }
}

#[test]
fn display_view_keeps_source_indices_after_sort_and_filter() {
    let result = sample_result();
    let sorted = build_display_view(&result, Some((0, SortDir::Asc)), "", "");
    assert_eq!(sorted.display_indices.as_slice(), &[1, 0, 2]);

    let filtered = build_display_view(&result, Some((0, SortDir::Desc)), "name", "bet");
    assert_eq!(filtered.visible_col_indices.as_slice(), &[1]);
    assert_eq!(filtered.display_indices.as_slice(), &[0]);
    assert!(filtered.cols_filtered);
    assert!(filtered.row_filtering);
}

#[test]
fn id_filter_matches_exact_integer_in_the_display_pipeline() {
    let result = sample_result();
    let filter = RowFilter::Integer(2);
    let view =
        build_display_view_cancellable(&result, None, "", &filter, true, &AtomicBool::new(false))
            .unwrap();

    assert_eq!(view.display_indices.as_slice(), &[0]);
    assert!(view.row_filtering);
}

#[test]
fn cancelled_display_view_stops_before_scanning_rows() {
    let cancelled = AtomicBool::new(true);
    let row_filter = RowFilter::Text("alpha".into());
    assert!(
        build_display_view_cancellable(
            &sample_result(),
            Some((0, SortDir::Asc)),
            "name",
            &row_filter,
            true,
            &cancelled,
        )
        .is_none()
    );
}

#[test]
fn display_view_cache_reuses_only_matching_revision_and_inputs() {
    let view = build_display_view(&sample_result(), None, "", "");
    let key = DisplayViewCacheKey {
        result_identity: 7,
        result_revision: 3,
        sort_by: None,
        column_filter: String::new(),
        row_filter: RowFilter::Text(String::new()),
        display_binary_16_as_uuid: true,
    };
    let cache = DisplayViewCache {
        key: key.clone(),
        view,
    };

    let cached = cache.get(&key).expect("same cache key should hit");
    assert!(Arc::ptr_eq(
        &cached.display_indices,
        &cache.view.display_indices
    ));

    let mut stale = key;
    stale.result_revision += 1;
    assert!(cache.get(&stale).is_none());

    let mut display_changed = cache.key.clone();
    display_changed.display_binary_16_as_uuid = false;
    assert!(cache.get(&display_changed).is_none());
}

#[test]
fn mixed_and_json_values_have_deterministic_direct_ordering() {
    let values = [
        Value::Json(serde_json::json!({"z": [3, 2, 1]})),
        Value::Text("plain".into()),
        Value::Bool(true),
    ];
    for (left_index, left) in values.iter().enumerate() {
        for (right_index, right) in values.iter().enumerate() {
            let forward = helpers::compare_values(Some(left), Some(right));
            let reverse = helpers::compare_values(Some(right), Some(left));
            assert_eq!(forward, reverse.reverse(), "{left_index} vs {right_index}");
        }
    }
    assert_eq!(
        helpers::compare_values(
            Some(&Value::Json(serde_json::json!([1, 2]))),
            Some(&Value::Json(serde_json::json!([1, 3]))),
        ),
        std::cmp::Ordering::Less
    );
}

#[test]
fn column_filter_matches_unicode_case_insensitively() {
    let mut result = sample_result();
    result.columns[1] = "ÜBERblick".into();

    let view = build_display_view(&result, None, "über", "");

    assert_eq!(view.visible_col_indices.as_slice(), &[1]);
}

#[test]
fn wide_results_cap_rendered_columns_but_filter_all_columns() {
    let column_count = MAX_COLUMNS_DISPLAY + 1;
    let mut values = vec![Value::Text(String::new()); column_count];
    values[column_count - 1] = Value::Text("needle".into());
    let result = QueryResult {
        columns: (0..column_count).map(|index| format!("c{index}")).collect(),
        column_types: vec!["TEXT".into(); column_count],
        rows: vec![Row { values }],
        affected_rows: 0,
        elapsed_ms: 1,
        warnings: Vec::new(),
        truncated: false,
    };

    let view = build_display_view(&result, None, "", "needle");

    assert_eq!(view.visible_col_indices.len(), MAX_COLUMNS_DISPLAY);
    assert_eq!(view.matched_col_count, column_count);
    assert!(view.columns_truncated);
    assert_eq!(view.display_indices.as_slice(), &[0]);
}

#[test]
fn unfiltered_wide_results_retain_only_rendered_column_indices() {
    let column_count = MAX_COLUMNS_DISPLAY + 128;
    let result = QueryResult {
        columns: (0..column_count).map(|index| format!("c{index}")).collect(),
        column_types: Vec::new(),
        rows: Vec::new(),
        affected_rows: 0,
        elapsed_ms: 1,
        warnings: Vec::new(),
        truncated: false,
    };

    let (matched_col_count, indices) =
        collect_matching_col_indices(&result, &[], false, &AtomicBool::new(false))
            .expect("unfiltered column collection should finish");

    assert_eq!(matched_col_count, column_count);
    assert_eq!(indices.len(), MAX_COLUMNS_DISPLAY);
    assert_eq!(indices.last(), Some(&(MAX_COLUMNS_DISPLAY - 1)));
}
