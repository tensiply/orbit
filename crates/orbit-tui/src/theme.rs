use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Clone)]
pub struct Palette {
    pub theme: Theme,
    /// Primary accent — tabs, borders, keyboard shortcuts
    pub accent: Color,
    /// Dimmed secondary text
    pub dim: Color,
    /// Normal body text (terminal default fg)
    pub text: Color,
    /// Selected-row background
    pub selected_bg: Color,
    /// Selected-row foreground
    pub selected_fg: Color,
    /// Table column headers, section headings
    pub label: Color,
    /// Status messages, alerts, emphasis in destructive contexts
    pub warning: Color,
    /// Positive state indicators (alive, running, stable)
    pub success: Color,
    /// Destructive action buttons, error messages
    pub danger: Color,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            theme: Theme::Dark,
            accent: Color::Cyan,
            dim: Color::DarkGray,
            text: Color::Reset,
            selected_bg: Color::DarkGray,
            selected_fg: Color::Reset,
            label: Color::Yellow,
            warning: Color::Yellow,
            success: Color::Green,
            danger: Color::Red,
        }
    }

    pub fn light() -> Self {
        Self {
            theme: Theme::Light,
            accent: Color::Blue,
            dim: Color::DarkGray,
            text: Color::Reset,
            selected_bg: Color::Blue,
            selected_fg: Color::White,
            label: Color::Yellow,
            warning: Color::Yellow,
            success: Color::Green,
            danger: Color::Red,
        }
    }

    pub fn detect() -> Self {
        // Explicit override via env var
        if let Ok(v) = std::env::var("ORBIT_THEME") {
            match v.to_lowercase().as_str() {
                "light" => return Self::light(),
                "dark" => return Self::dark(),
                _ => {}
            }
        }
        // TERM_BACKGROUND set by Alacritty, Warp, kitty, and others
        if let Ok(v) = std::env::var("TERM_BACKGROUND") {
            if v.eq_ignore_ascii_case("light") {
                return Self::light();
            }
            if v.eq_ignore_ascii_case("dark") {
                return Self::dark();
            }
        }
        // COLORFGBG: "fg;bg" — bg == 7 or >= 9 typically means a light background
        if let Ok(v) = std::env::var("COLORFGBG") {
            if let Some(bg) = v.split(';').last().and_then(|s| s.parse::<u8>().ok()) {
                if bg == 7 || bg >= 9 {
                    return Self::light();
                }
            }
        }
        Self::dark()
    }
}
