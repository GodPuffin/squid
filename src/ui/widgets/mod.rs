use ratatui::widgets::Block;

use crate::theme::Theme;

pub fn panel_block(title: &str, active: bool, theme: Theme) -> Block<'_> {
    theme.panel_block(title, active)
}
