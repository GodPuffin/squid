use anyhow::{Context, Result};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ColorScheme {
    Dark,
    Light,
    Monokai,
    SolarizedDark,
    SolarizedLight,
    Dracula,
}

impl ColorScheme {
    pub const ALL: [Self; 6] = [
        Self::Dark,
        Self::Light,
        Self::Monokai,
        Self::SolarizedDark,
        Self::SolarizedLight,
        Self::Dracula,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Monokai => "monokai",
            Self::SolarizedDark => "solarized dark",
            Self::SolarizedLight => "solarized light",
            Self::Dracula => "dracula",
        }
    }

    pub fn cycle(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|scheme| *scheme == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn from_storage(value: &str) -> Result<Self> {
        match value {
            "dark" => Ok(Self::Dark),
            "light" => Ok(Self::Light),
            "monokai" => Ok(Self::Monokai),
            "solarized_dark" => Ok(Self::SolarizedDark),
            "solarized_light" => Ok(Self::SolarizedLight),
            "dracula" => Ok(Self::Dracula),
            other => anyhow::bail!("invalid color scheme value: {other}"),
        }
    }

    pub fn to_storage(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Monokai => "monokai",
            Self::SolarizedDark => "solarized_dark",
            Self::SolarizedLight => "solarized_light",
            Self::Dracula => "dracula",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub scheme: ColorScheme,
    pub background: Option<Color>,
    pub foreground: Color,
    pub muted: Color,
    pub muted_weak: Color,
    pub accent: Color,
    pub border_active: Color,
    pub border_inactive: Color,
    pub tab_active_fg: Color,
    pub tab_active_bg: Color,
    pub tab_inactive_fg: Color,
    pub tab_inactive_bg: Color,
    pub selection_fg: Color,
    pub selection_bg: Color,
    pub list_highlight_fg: Color,
    pub list_highlight_bg: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub overlay_border: Color,
    pub search_table: Color,
    pub search_match: Color,
    pub syntax_keyword: Color,
    pub syntax_string: Color,
    pub syntax_number: Color,
    pub syntax_comment: Color,
    pub syntax_ident: Color,
    pub button_label_fg: Color,
    pub emphasis: Color,
    pub empty: Color,
}

impl Theme {
    pub fn from_scheme(scheme: ColorScheme) -> Self {
        match scheme {
            ColorScheme::Dark => dark(),
            ColorScheme::Light => light(),
            ColorScheme::Monokai => monokai(),
            ColorScheme::SolarizedDark => solarized_dark(),
            ColorScheme::SolarizedLight => solarized_light(),
            ColorScheme::Dracula => dracula(),
        }
    }

    pub fn fg_style(self) -> Style {
        Style::default().fg(self.foreground)
    }

    pub fn muted_style(self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn muted_weak_style(self) -> Style {
        Style::default().fg(self.muted_weak)
    }

    pub fn emphasis_style(self) -> Style {
        Style::default()
            .fg(self.emphasis)
            .add_modifier(Modifier::BOLD)
    }

    pub fn success_style(self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn success_bold_style(self) -> Style {
        self.success_style().add_modifier(Modifier::BOLD)
    }

    pub fn error_style(self) -> Style {
        Style::default().fg(self.error).add_modifier(Modifier::BOLD)
    }

    pub fn warning_style(self) -> Style {
        Style::default()
            .fg(self.warning)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selection_style(self) -> Style {
        Style::default()
            .fg(self.selection_fg)
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn list_highlight_style(self) -> Style {
        Style::default()
            .fg(self.list_highlight_fg)
            .bg(self.list_highlight_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tab_style(self, active: bool) -> Style {
        if active {
            Style::default()
                .fg(self.tab_active_fg)
                .bg(self.tab_active_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.tab_inactive_fg)
                .bg(self.tab_inactive_bg)
                .add_modifier(Modifier::BOLD)
        }
    }

    pub fn button_style(self, background: Color) -> Style {
        Style::default()
            .fg(self.button_label_fg)
            .bg(background)
            .add_modifier(Modifier::BOLD)
    }

    pub fn overlay_border_style(self) -> Style {
        Style::default()
            .fg(self.overlay_border)
            .add_modifier(Modifier::BOLD)
    }

    pub fn search_table_style(self) -> Style {
        Style::default()
            .fg(self.search_table)
            .add_modifier(Modifier::BOLD)
    }

    pub fn search_match_style(self) -> Style {
        Style::default()
            .fg(self.search_match)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    pub fn syntax_keyword_style(self) -> Style {
        Style::default()
            .fg(self.syntax_keyword)
            .add_modifier(Modifier::BOLD)
    }

    pub fn syntax_string_style(self) -> Style {
        Style::default().fg(self.syntax_string)
    }

    pub fn syntax_number_style(self) -> Style {
        Style::default().fg(self.syntax_number)
    }

    pub fn syntax_comment_style(self) -> Style {
        Style::default().fg(self.syntax_comment)
    }

    pub fn syntax_ident_style(self) -> Style {
        Style::default().fg(self.syntax_ident)
    }

    pub fn empty_style(self) -> Style {
        Style::default()
            .fg(self.empty)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn panel_block<'a>(self, title: &'a str, active: bool) -> Block<'a> {
        let border_style = if active {
            Style::default()
                .fg(self.border_active)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.border_inactive)
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .title(title.to_string())
            .border_style(border_style);

        if let Some(background) = self.background {
            block = block.style(Style::default().fg(self.foreground).bg(background));
        } else {
            block = block.style(Style::default().fg(self.foreground));
        }

        block
    }

    pub fn fill_style(self) -> Style {
        match self.background {
            Some(background) => Style::default().fg(self.foreground).bg(background),
            None => Style::default().fg(self.foreground),
        }
    }
}

fn dark() -> Theme {
    Theme {
        scheme: ColorScheme::Dark,
        background: None,
        foreground: Color::White,
        muted: Color::Gray,
        muted_weak: Color::DarkGray,
        accent: Color::Cyan,
        border_active: Color::Cyan,
        border_inactive: Color::DarkGray,
        tab_active_fg: Color::Black,
        tab_active_bg: Color::Cyan,
        tab_inactive_fg: Color::White,
        tab_inactive_bg: Color::DarkGray,
        selection_fg: Color::Black,
        selection_bg: Color::Yellow,
        list_highlight_fg: Color::Black,
        list_highlight_bg: Color::Cyan,
        success: Color::LightGreen,
        error: Color::LightRed,
        warning: Color::LightYellow,
        overlay_border: Color::LightCyan,
        search_table: Color::Cyan,
        search_match: Color::LightYellow,
        syntax_keyword: Color::Cyan,
        syntax_string: Color::LightGreen,
        syntax_number: Color::LightMagenta,
        syntax_comment: Color::DarkGray,
        syntax_ident: Color::White,
        button_label_fg: Color::Black,
        emphasis: Color::White,
        empty: Color::DarkGray,
    }
}

fn light() -> Theme {
    Theme {
        scheme: ColorScheme::Light,
        background: Some(rgb(250, 250, 250)),
        foreground: rgb(24, 24, 24),
        muted: rgb(96, 96, 96),
        muted_weak: rgb(140, 140, 140),
        accent: rgb(0, 102, 204),
        border_active: rgb(0, 102, 204),
        border_inactive: rgb(180, 180, 180),
        tab_active_fg: Color::White,
        tab_active_bg: rgb(0, 102, 204),
        tab_inactive_fg: rgb(24, 24, 24),
        tab_inactive_bg: rgb(220, 220, 220),
        selection_fg: Color::White,
        selection_bg: rgb(0, 102, 204),
        list_highlight_fg: Color::White,
        list_highlight_bg: rgb(0, 102, 204),
        success: rgb(0, 128, 0),
        error: rgb(180, 0, 0),
        warning: rgb(180, 120, 0),
        overlay_border: rgb(0, 102, 204),
        search_table: rgb(0, 102, 204),
        search_match: rgb(180, 120, 0),
        syntax_keyword: rgb(0, 0, 170),
        syntax_string: rgb(0, 128, 0),
        syntax_number: rgb(128, 0, 128),
        syntax_comment: rgb(140, 140, 140),
        syntax_ident: rgb(24, 24, 24),
        button_label_fg: Color::White,
        emphasis: rgb(24, 24, 24),
        empty: rgb(140, 140, 140),
    }
}

fn monokai() -> Theme {
    Theme {
        scheme: ColorScheme::Monokai,
        background: Some(rgb(39, 40, 34)),
        foreground: rgb(248, 248, 242),
        muted: rgb(117, 113, 94),
        muted_weak: rgb(98, 94, 76),
        accent: rgb(166, 226, 46),
        border_active: rgb(166, 226, 46),
        border_inactive: rgb(73, 72, 62),
        tab_active_fg: rgb(39, 40, 34),
        tab_active_bg: rgb(166, 226, 46),
        tab_inactive_fg: rgb(248, 248, 242),
        tab_inactive_bg: rgb(73, 72, 62),
        selection_fg: rgb(39, 40, 34),
        selection_bg: rgb(230, 219, 116),
        list_highlight_fg: rgb(39, 40, 34),
        list_highlight_bg: rgb(166, 226, 46),
        success: rgb(166, 226, 46),
        error: rgb(249, 38, 114),
        warning: rgb(230, 219, 116),
        overlay_border: rgb(102, 217, 239),
        search_table: rgb(166, 226, 46),
        search_match: rgb(230, 219, 116),
        syntax_keyword: rgb(249, 38, 114),
        syntax_string: rgb(230, 219, 116),
        syntax_number: rgb(174, 129, 255),
        syntax_comment: rgb(117, 113, 94),
        syntax_ident: rgb(248, 248, 242),
        button_label_fg: rgb(39, 40, 34),
        emphasis: rgb(248, 248, 242),
        empty: rgb(117, 113, 94),
    }
}

fn solarized_dark() -> Theme {
    Theme {
        scheme: ColorScheme::SolarizedDark,
        background: Some(rgb(0, 43, 54)),
        foreground: rgb(131, 148, 150),
        muted: rgb(88, 110, 117),
        muted_weak: rgb(7, 54, 66),
        accent: rgb(42, 161, 152),
        border_active: rgb(42, 161, 152),
        border_inactive: rgb(7, 54, 66),
        tab_active_fg: rgb(0, 43, 54),
        tab_active_bg: rgb(42, 161, 152),
        tab_inactive_fg: rgb(131, 148, 150),
        tab_inactive_bg: rgb(7, 54, 66),
        selection_fg: rgb(0, 43, 54),
        selection_bg: rgb(181, 137, 0),
        list_highlight_fg: rgb(0, 43, 54),
        list_highlight_bg: rgb(42, 161, 152),
        success: rgb(133, 153, 0),
        error: rgb(220, 50, 47),
        warning: rgb(181, 137, 0),
        overlay_border: rgb(38, 139, 210),
        search_table: rgb(42, 161, 152),
        search_match: rgb(181, 137, 0),
        syntax_keyword: rgb(38, 139, 210),
        syntax_string: rgb(133, 153, 0),
        syntax_number: rgb(211, 54, 130),
        syntax_comment: rgb(88, 110, 117),
        syntax_ident: rgb(147, 161, 161),
        button_label_fg: rgb(0, 43, 54),
        emphasis: rgb(147, 161, 161),
        empty: rgb(88, 110, 117),
    }
}

fn solarized_light() -> Theme {
    Theme {
        scheme: ColorScheme::SolarizedLight,
        background: Some(rgb(253, 246, 227)),
        foreground: rgb(101, 123, 131),
        muted: rgb(147, 161, 161),
        muted_weak: rgb(181, 137, 0),
        accent: rgb(38, 139, 210),
        border_active: rgb(38, 139, 210),
        border_inactive: rgb(147, 161, 161),
        tab_active_fg: rgb(253, 246, 227),
        tab_active_bg: rgb(38, 139, 210),
        tab_inactive_fg: rgb(101, 123, 131),
        tab_inactive_bg: rgb(238, 232, 213),
        selection_fg: rgb(253, 246, 227),
        selection_bg: rgb(38, 139, 210),
        list_highlight_fg: rgb(253, 246, 227),
        list_highlight_bg: rgb(38, 139, 210),
        success: rgb(133, 153, 0),
        error: rgb(220, 50, 47),
        warning: rgb(181, 137, 0),
        overlay_border: rgb(42, 161, 152),
        search_table: rgb(38, 139, 210),
        search_match: rgb(181, 137, 0),
        syntax_keyword: rgb(38, 139, 210),
        syntax_string: rgb(133, 153, 0),
        syntax_number: rgb(211, 54, 130),
        syntax_comment: rgb(147, 161, 161),
        syntax_ident: rgb(101, 123, 131),
        button_label_fg: rgb(253, 246, 227),
        emphasis: rgb(101, 123, 131),
        empty: rgb(147, 161, 161),
    }
}

fn dracula() -> Theme {
    Theme {
        scheme: ColorScheme::Dracula,
        background: Some(rgb(40, 42, 54)),
        foreground: rgb(248, 248, 242),
        muted: rgb(98, 114, 164),
        muted_weak: rgb(68, 71, 90),
        accent: rgb(189, 147, 249),
        border_active: rgb(189, 147, 249),
        border_inactive: rgb(68, 71, 90),
        tab_active_fg: rgb(40, 42, 54),
        tab_active_bg: rgb(189, 147, 249),
        tab_inactive_fg: rgb(248, 248, 242),
        tab_inactive_bg: rgb(68, 71, 90),
        selection_fg: rgb(40, 42, 54),
        selection_bg: rgb(255, 184, 108),
        list_highlight_fg: rgb(40, 42, 54),
        list_highlight_bg: rgb(189, 147, 249),
        success: rgb(80, 250, 123),
        error: rgb(255, 85, 85),
        warning: rgb(255, 184, 108),
        overlay_border: rgb(139, 233, 253),
        search_table: rgb(189, 147, 249),
        search_match: rgb(255, 184, 108),
        syntax_keyword: rgb(255, 121, 198),
        syntax_string: rgb(241, 250, 140),
        syntax_number: rgb(189, 147, 249),
        syntax_comment: rgb(98, 114, 164),
        syntax_ident: rgb(248, 248, 242),
        button_label_fg: rgb(40, 42, 54),
        emphasis: rgb(248, 248, 242),
        empty: rgb(98, 114, 164),
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

pub fn parse_color_scheme(value: &str) -> Result<ColorScheme> {
    ColorScheme::from_storage(value).with_context(|| format!("invalid color scheme: {value}"))
}

#[cfg(test)]
mod tests;
