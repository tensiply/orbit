pub fn bin_available(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn truncate_desc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max.saturating_sub(3))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}

pub fn section(title: &str) {
    println!("{title}");
}

pub fn dim(s: &str) -> String {
    format!("\x1b[2m{s}\x1b[0m")
}

pub fn check<E: std::fmt::Display>(label: &str, result: Result<(), E>, hint: Option<&str>) {
    match result {
        Ok(()) => println!("  \x1b[32m✓\x1b[0m  {label}"),
        Err(e) => {
            println!("  \x1b[31m✗\x1b[0m  {label}  — {e}");
            if let Some(h) = hint {
                println!("      \x1b[2m{h}\x1b[0m");
            }
        }
    }
}
