use std::fs;
use std::io;
use std::path::Path;

pub fn generate_tree(path: &Path, prefix: &str) -> io::Result<String> {
    let mut output = String::new();
    let entries = fs::read_dir(path)?;

    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());

    let len = entries.len();
    for (i, entry) in entries.into_iter().enumerate() {
        let file_name = entry.file_name().into_string().unwrap_or_default();
        let is_last = i == len - 1;
        let connector = if is_last { "└── " } else { "├── " };
        output.push_str(&format!("{}{}{}\n", prefix, connector, file_name));
        let new_path = entry.path();
        if new_path.is_dir() {
            let new_prefix = if is_last { format!("{}    ", prefix) } else { format!("{}│   ", prefix) };
            output.push_str(&generate_tree(&new_path, &new_prefix)?);
        }
    }
    Ok(output)
}