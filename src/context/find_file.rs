use std::process::Command;

pub fn find_files(names: &[&str]) -> Vec<String> {
    let output = Command::new("fd")
        .args([
            "-a",
            &format_names_arg(names)
        ])
        .output()
        .expect("Failed to execute find command");

    String::from_utf8(output.stdout)
        .expect("Failed to parse command result as UTF-8")
        .lines()
        .map(|it| it.to_string())
        .collect()
}

fn format_names_arg(names: &[&str]) -> String {
    names.iter()
        .map(|name| format!("^{name}.js$"))
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_names_arg() {
        assert_eq!("^one.js$|^two.js$", format_names_arg(&["one", "two"]));
    }
}
