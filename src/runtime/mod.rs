mod input;
mod mouse;
mod terminal;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::Rect;

use crate::app::{Action, App};

pub fn run(path: PathBuf) -> Result<()> {
    let mut terminal = terminal::setup_terminal()?;
    let result = run_loop(&mut terminal, path);
    terminal::restore_terminal(&mut terminal)?;
    result
}

fn run_loop(terminal: &mut terminal::TerminalHandle, path: PathBuf) -> Result<()> {
    let mut app = App::load(path)?;
    let mut mouse_state = mouse::MouseState::default();

    loop {
        let size = terminal.size()?;
        let viewport = crate::ui::viewport_sizes(Rect::new(0, 0, size.width, size.height));
        app.set_viewport_sizes(
            viewport.row_limit,
            viewport.schema_page_lines,
            viewport.detail_value_width,
            viewport.detail_value_height,
        )?;
        let layout = crate::ui::layout_info(Rect::new(0, 0, size.width, size.height), &app);
        if let Some(sql) = &layout.sql {
            app.set_sql_viewport_sizes(
                sql.editor.height.saturating_sub(2) as usize,
                sql.history.height.saturating_sub(2) as usize,
                sql.results.height.saturating_sub(3) as usize,
            );
        }
        terminal.draw(|frame| crate::ui::render(frame, &app))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = input::action_for_key(&app, key);
                if matches!(action, Action::Quit) && app.modal.is_none() {
                    break;
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
