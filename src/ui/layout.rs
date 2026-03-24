use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};

use crate::app::{App, AppMode};

pub struct ViewportSizes {
    pub row_limit: usize,
    pub schema_page_lines: usize,
    pub detail_value_width: usize,
    pub detail_value_height: usize,
}

pub struct LayoutInfo {
    pub header: Rect,
    pub header_tabs: HeaderTabRects,
    pub tables: Rect,
    pub content: Rect,
    pub footer: Rect,
    pub search_box: Option<Rect>,
    pub search_results: Option<Rect>,
    pub sql: Option<SqlRects>,
    pub detail: Option<DetailRects>,
    pub filter_modal: Option<FilterModalRects>,
    pub modal: Option<ModalRects>,
}

pub struct HeaderTabRects {
    pub browse: Rect,
    pub sql: Rect,
    pub path: Rect,
    pub run: Rect,
    pub quit: Rect,
}

impl HeaderTabRects {
    fn empty() -> Self {
        Self {
            browse: Rect::default(),
            sql: Rect::default(),
            path: Rect::default(),
            run: Rect::default(),
            quit: Rect::default(),
        }
    }
}

pub struct SqlRects {
    pub editor: Rect,
    pub history: Rect,
    pub results: Rect,
    pub completion: Option<Rect>,
}

pub struct ModalRects {
    pub area: Rect,
    pub header: Rect,
    pub columns: Rect,
    pub sort_candidates: Rect,
    pub sort_stack: Rect,
    pub footer: Rect,
}

pub struct FilterModalRects {
    pub area: Rect,
    pub header: Rect,
    pub columns: Rect,
    pub modes: Rect,
    pub draft: Rect,
    pub active: Rect,
    pub footer: Rect,
}

pub struct DetailRects {
    pub area: Rect,
    pub header: Rect,
    pub fields: Rect,
    pub value: Rect,
    pub footer: Rect,
}

pub fn viewport_sizes(area: Rect) -> ViewportSizes {
    let areas = root_layout(area);
    let body = body_layout(areas[1], 24);
    let content_height = body[1].height;
    let detail = detail_rects(area);
    let value_width = detail.value.width.saturating_sub(2).max(1) as usize;
    let value_height = detail.value.height.saturating_sub(2).max(1) as usize;

    ViewportSizes {
        row_limit: content_height.saturating_sub(3).max(1) as usize,
        schema_page_lines: content_height.saturating_sub(2).max(1) as usize,
        detail_value_width: value_width,
        detail_value_height: value_height,
    }
}

pub fn layout_info(area: Rect, app: &App) -> LayoutInfo {
    if app.is_home() {
        let home = home_layout(area);
        return LayoutInfo {
            header: home.header,
            header_tabs: HeaderTabRects::empty(),
            tables: home.recents,
            content: home.content,
            footer: home.footer,
            search_box: None,
            search_results: None,
            sql: None,
            detail: None,
            filter_modal: None,
            modal: None,
        };
    }

    let areas = root_layout(area);
    let body = if app.mode == AppMode::Browse {
        body_layout(areas[1], app.table_pane_width())
    } else {
        vec![Rect::default(), areas[1]]
    };

    LayoutInfo {
        header: areas[0],
        header_tabs: header_tab_rects(areas[0]),
        tables: body[0],
        content: body[1],
        footer: areas[2],
        search_box: app.search.as_ref().map(|_| search_layout(body[1])[0]),
        search_results: app.search.as_ref().map(|_| search_layout(body[1])[1]),
        sql: (app.mode == AppMode::Sql).then(|| {
            let mut sql = sql_layout(body[1]);
            if !app.sql_completion_items().is_empty() {
                let (line, _) = app.sql_cursor_line_col();
                sql.completion = Some(sql_completion_rect(
                    sql.editor,
                    line.saturating_sub(app.sql.editor_scroll),
                    app.sql_cursor_screen_col(),
                ));
            }
            sql
        }),
        detail: app.detail.as_ref().map(|_| detail_rects(area)),
        filter_modal: app.filter_modal.as_ref().map(|_| filter_modal_rects(area)),
        modal: app.modal.as_ref().map(|_| modal_rects(area)),
    }
}

struct HomeLayout {
    header: Rect,
    content: Rect,
    recents: Rect,
    footer: Rect,
}

fn home_layout(area: Rect) -> HomeLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

    let recents = centered_rect(vertical[2], 34, 100);
    let content = centered_rect(vertical[3], 60, 100);
    let footer = centered_rect(vertical[4], 70, 100);

    HomeLayout {
        header: vertical[1],
        content,
        recents,
        footer,
    }
}

pub fn list_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    if column < area.x + 1 || column >= area.x + area.width.saturating_sub(1) {
        return None;
    }
    if row < area.y + 1 || row >= area.y + area.height.saturating_sub(1) {
        return None;
    }
    Some((row - area.y - 1) as usize)
}

pub fn list_scroll_offset(area: Rect, selected: usize, item_count: usize) -> usize {
    let visible_rows = area.height.saturating_sub(2) as usize;
    if visible_rows == 0 || item_count <= visible_rows {
        return 0;
    }

    selected
        .saturating_sub(visible_rows.saturating_sub(1))
        .min(item_count.saturating_sub(visible_rows))
}

pub fn home_recent_row_at(
    area: Rect,
    column: u16,
    row: u16,
    selected: usize,
    item_count: usize,
) -> Option<usize> {
    let relative_index = list_row_at(area, column, row)?;
    let absolute_index = list_scroll_offset(area, selected, item_count) + relative_index;
    (absolute_index < item_count).then_some(absolute_index)
}

pub fn table_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    if column < area.x + 1 || column >= area.x + area.width.saturating_sub(1) {
        return None;
    }
    if row < area.y + 2 || row >= area.y + area.height.saturating_sub(1) {
        return None;
    }
    Some((row - area.y - 2) as usize)
}

pub fn search_result_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    if column < area.x + 1 || column >= area.x + area.width.saturating_sub(1) {
        return None;
    }
    if row < area.y + 2 || row >= area.y + area.height.saturating_sub(1) {
        return None;
    }
    Some((row - area.y - 2) as usize)
}

pub fn root_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area)
        .to_vec()
}

pub fn body_layout(area: Rect, tables_width: u16) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(tables_width), Constraint::Min(20)])
        .split(area)
        .to_vec()
}

pub fn search_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(6)])
        .split(area)
        .to_vec()
}

pub fn sql_layout(area: Rect) -> SqlRects {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(vertical[1]);

    SqlRects {
        editor: vertical[0],
        history: bottom[0],
        results: bottom[1],
        completion: None,
    }
}

pub fn header_tab_rects(area: Rect) -> HeaderTabRects {
    let inner_x = area.x.saturating_add(2);
    let inner_y = area.y.saturating_add(1);
    let browse = Rect::new(inner_x, inner_y, 10, 1);
    let sql = Rect::new(inner_x.saturating_add(11), inner_y, 7, 1);
    let quit_width = 8;
    let run_width = 7;
    let quit_x = area.x + area.width.saturating_sub(quit_width + 2);
    let run_x = quit_x.saturating_sub(run_width + 1);
    let path_x = sql.x + sql.width + 2;
    let path_width = run_x.saturating_sub(path_x + 1);
    let path = Rect::new(path_x, inner_y, path_width, 1);
    let run = Rect::new(run_x, inner_y, run_width, 1);
    let quit = Rect::new(quit_x, inner_y, quit_width, 1);
    HeaderTabRects {
        browse,
        sql,
        path,
        run,
        quit,
    }
}

pub fn sql_completion_rect(editor_area: Rect, cursor_line: usize, cursor_col: usize) -> Rect {
    let popup_height = 8;
    let popup_width = 24;
    let x = editor_area
        .x
        .saturating_add(1)
        .saturating_add(cursor_col.min(editor_area.width.saturating_sub(4) as usize) as u16);
    let y = editor_area
        .y
        .saturating_add(1)
        .saturating_add(cursor_line as u16 + 1);

    Rect::new(
        x.min(editor_area.x + editor_area.width.saturating_sub(popup_width + 1)),
        y.min(editor_area.y + editor_area.height.saturating_sub(popup_height + 1)),
        popup_width.min(editor_area.width.saturating_sub(2)),
        popup_height.min(editor_area.height.saturating_sub(2)),
    )
}

pub fn modal_rects(area: Rect) -> ModalRects {
    let modal_area = centered_rect(area, 84, 70);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(modal_area);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(layout[1]);
    ModalRects {
        area: modal_area,
        header: layout[0],
        columns: panes[0],
        sort_candidates: panes[1],
        sort_stack: panes[2],
        footer: layout[2],
    }
}

pub fn filter_modal_rects(area: Rect) -> FilterModalRects {
    let modal_area = centered_rect(area, 88, 72);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(modal_area);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(26),
            Constraint::Percentage(44),
        ])
        .split(layout[1]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(5)])
        .split(panes[2]);

    FilterModalRects {
        area: modal_area,
        header: layout[0],
        columns: panes[0],
        modes: panes[1],
        draft: right[0],
        active: right[1],
        footer: layout[2],
    }
}

pub fn detail_rects(area: Rect) -> DetailRects {
    let modal_area = centered_rect(area, 88, 76);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(modal_area);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(layout[1]);

    DetailRects {
        area: modal_area,
        header: layout[0],
        fields: panes[0],
        value: panes[1],
        footer: layout[2],
    }
}

pub fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .flex(Flex::Center)
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .flex(Flex::Center)
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::{Rect, home_recent_row_at, list_scroll_offset};

    #[test]
    fn list_scroll_offset_keeps_selected_row_visible() {
        let area = Rect::new(0, 0, 20, 9);

        assert_eq!(list_scroll_offset(area, 0, 12), 0);
        assert_eq!(list_scroll_offset(area, 6, 12), 0);
        assert_eq!(list_scroll_offset(area, 7, 12), 1);
        assert_eq!(list_scroll_offset(area, 11, 12), 5);
    }

    #[test]
    fn home_recent_row_at_applies_scroll_offset() {
        let area = Rect::new(0, 0, 20, 9);

        assert_eq!(home_recent_row_at(area, 1, 1, 8, 12), Some(2));
        assert_eq!(home_recent_row_at(area, 1, 7, 8, 12), Some(8));
    }
}
