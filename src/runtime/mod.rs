mod input;
mod mouse;
mod terminal;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::Rect;

use crate::app::{Action, App};

pub fn run(path: Option<PathBuf>) -> Result<()> {
    let mut terminal = terminal::setup_terminal()?;
    let result = run_loop(&mut terminal, path);
    terminal::restore_terminal(&mut terminal)?;
    result
}

fn run_loop(terminal: &mut terminal::TerminalHandle, path: Option<PathBuf>) -> Result<()> {
    let mut app = App::load(path)?;
    let mut mouse_state = mouse::MouseState::default();

    loop {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        let viewport = crate::ui::viewport_sizes(area);
        app.set_viewport_sizes(
            viewport.row_limit,
            viewport.schema_page_lines,
            viewport.detail_value_width,
            viewport.detail_value_height,
        )?;
        let mut layout = crate::ui::layout_info(area, &app);
        app.sync_search_results_view_width(
            layout
                .search_results
                .map(|area| area.width as usize)
                .unwrap_or(0),
        );
        if let Some(sql) = &layout.sql {
            app.set_sql_viewport_sizes(
                sql.editor.height.saturating_sub(2) as usize,
                sql.editor.width.saturating_sub(2) as usize,
                sql.history.height.saturating_sub(2) as usize,
                sql.results.height.saturating_sub(3) as usize,
            );
            layout.refresh_view_dependent_rects(&app);
        }
        terminal.draw(|frame| crate::ui::render(frame, &app, &layout))?;

        let ran_pending_work = app.run_pending_work()?;
        let poll_timeout = if ran_pending_work && !app.has_pending_work() {
            Duration::from_millis(0)
        } else if app.has_pending_work() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(200)
        };

        if !event::poll(poll_timeout)? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = input::action_for_key(&app, key);
                if matches!(action, Action::Quit) {
                    if app.request_quit()? {
                        break;
                    }
                    continue;
                }
                app.handle(action)?;
            }
            Event::Mouse(event) => {
                let should_quit = mouse::handle_mouse_event(
                    &mut app,
                    &layout,
                    event,
                    &mut mouse_state,
                    Instant::now(),
                )?;
                if should_quit {
                    break;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
