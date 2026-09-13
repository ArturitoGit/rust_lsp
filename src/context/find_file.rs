use std::process::Command;
use std::env;

pub fn find_files(file_name: &str) -> Vec<String> {

    let name_with_suffix = format!("{file_name}.js");
    let current_dir = format!("{}", env::current_dir().unwrap().display());

    let output = Command::new("find")
        .args([
            &current_dir, // Use current directory in command to get absolute paths in result
            "-type", "f",
            "-name", &name_with_suffix
        ])
        .output()
        .expect("Failed to execute find command");

    String::from_utf8(output.stdout)
        .expect("Failed to parse command result as UTF-8")
        .lines()
        .map(|it| it.to_string())
        .collect()
}
