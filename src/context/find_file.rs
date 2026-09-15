use std::process::Command;

pub fn find_files(file_name: &str) -> Vec<String> {

    let name_with_suffix = format!("{file_name}.js");

    let output = Command::new("fd").args([ "-a", &name_with_suffix ])
        .output()
        .expect("Failed to execute find command");

    String::from_utf8(output.stdout)
        .expect("Failed to parse command result as UTF-8")
        .lines()
        .map(|it| it.to_string())
        .collect()
}
