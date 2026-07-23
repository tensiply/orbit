const BUILTIN_COMMANDS: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/builtin_commands.rs"));

pub fn all() -> &'static [(&'static str, &'static str)] {
    BUILTIN_COMMANDS
}

pub fn find(name: &str) -> Option<&'static str> {
    BUILTIN_COMMANDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}
