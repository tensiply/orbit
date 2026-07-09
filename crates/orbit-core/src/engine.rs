use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Engine {
    #[default]
    Opencode,
    Gemini,
    Claude,
}

impl Engine {
    pub fn as_str(&self) -> &'static str {
        match self {
            Engine::Opencode => "opencode",
            Engine::Gemini => "gemini",
            Engine::Claude => "claude",
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Engine {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "opencode" => Ok(Engine::Opencode),
            "gemini" => Ok(Engine::Gemini),
            "claude" => Ok(Engine::Claude),
            other => Err(format!("unknown engine: {other}")),
        }
    }
}
