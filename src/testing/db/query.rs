use super::{FilterClause, FilterMode, SortClause, build_filter_where, build_order_by};
use rusqlite::types::Value;

#[test]
fn build_filter_where_uses_all_clauses() {
    let clauses = vec![
        FilterClause {
            column_name: "name".into(),
            mode: FilterMode::Contains,
            value: "sam".into(),
        },
        FilterClause {
            column_name: "active".into(),
            mode: FilterMode::IsTrue,
            value: String::new(),
        },
    ];

    let (sql, params) = build_filter_where(&clauses);
    assert!(sql.contains("\"name\" LIKE ?"));
    assert!(sql.contains("CAST(\"active\" AS INTEGER) <> 0"));
    assert_eq!(params, vec![Value::Text("%sam%".into())]);
}

#[test]
fn build_order_by_keeps_sort_priority() {
    let clauses = vec![
        SortClause {
            column_name: "last_name".into(),
            descending: false,
        },
        SortClause {
            column_name: "created_at".into(),
            descending: true,
        },
    ];

    assert_eq!(
        build_order_by(&clauses),
        " ORDER BY \"last_name\" ASC, \"created_at\" DESC"
    );
}
