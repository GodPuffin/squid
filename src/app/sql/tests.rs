use super::App;

#[test]
fn sql_query_lines_preserve_trailing_empty_line() {
    let mut app = App::load(None).expect("load app");
    app.sql.query = "SELECT\n".to_string();
    app.sql.cursor = app.sql.query.len();

    assert_eq!(
        app.sql_query_lines(),
        vec!["SELECT".to_string(), String::new()]
    );
    assert_eq!(app.sql_cursor_line_col(), (1, 0));
}
