use orbit_core::channel::Channel;
use std::io::IsTerminal;

pub fn print() {
    if !std::io::stdout().is_terminal() {
        return;
    }
    let v = env!("CARGO_PKG_VERSION");
    // Non-stable channels carry their identifier in the banner: "· CANARY ·".
    let channel = match Channel::current().label() {
        Some(label) => format!("  ·  {label}"),
        None => String::new(),
    };
    println!();
    println!("\x1b[1;36m   ___  ____  ____ ___ _____ \x1b[0m");
    println!("\x1b[1;36m  / _ \\|  _ \\| __ )_ _|_   _|\x1b[0m");
    println!("\x1b[1;36m | | | | |_) |  _ \\| |  | |  \x1b[0m");
    println!("\x1b[1;36m | |_| |  _ <| |_) | |  | |  \x1b[0m");
    println!("\x1b[1;36m  \\___/|_| \\_\\____/___| |_|  \x1b[0m");
    println!("\x1b[2m  AI Ecosystem CLI{channel}  ·  v{v}\x1b[0m");
    println!("\x1b[2m  ─────────────────────────────\x1b[0m");
    println!();
}
