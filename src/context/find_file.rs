use std::process::Command;

pub fn find_files(names: &[&str]) -> Vec<String> {

    let names_arg = names.iter()
        .map(|name| format!("{name}.js"))
        .collect::<Vec<_>>()
        .join("|");

    let output = Command::new("fd").args([ "-a", &names_arg ])
        .output()
        .expect("Failed to execute find command");

    String::from_utf8(output.stdout)
        .expect("Failed to parse command result as UTF-8")
        .lines()
        .map(|it| it.to_string())
        .collect()
}
