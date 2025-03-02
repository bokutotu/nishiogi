//! # Directory Tree Generator
//!
//! This module provides a function to generate a textual representation of a directory tree.
//! It recursively traverses a given directory, allowing you to ignore files or directories
//! that match provided regular expressions, and optionally limits the depth of the tree.
//!
//! **Note:** This function will panic if it fails to read a directory (for example, due to
//! insufficient permissions or a non-existent path).

use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

use regex::Regex;

/// Generates a textual tree representation of the directory structure starting at `path`.
///
/// The function recursively lists the contents of the directory. The `prefix` is used to
/// format the tree structure. The optional `ignore` slice contains regular expressions to filter
/// out file or directory names. The optional `depth` limits the recursion depth.
///
/// **Panics:** This function will panic if an I/O error occurs while reading a directory.
/// For example, if `fs::read_dir(path)` fails, the function will panic with the message
/// "Failed to read directory".
///
/// # Arguments
///
/// * `path` - The root directory path for which to generate the tree.
/// * `prefix` - A string used as a prefix for each line in the tree output.
/// * `ignore` - An optional slice of `Regex` patterns. Entries matching any pattern will be ignored.
/// * `depth` - An optional maximum recursion depth. A value of `Some(0)` returns an empty string.
///
/// # Returns
///
/// A `String` containing the tree representation of the directory.
pub fn generate_tree(
    path: &Path,
    prefix: &str,
    ignore: Option<&[Regex]>,
    depth: Option<usize>,
) -> String {
    // If ignore patterns weren't provided, try to use .gitignore patterns
    let patterns = match ignore {
        Some(patterns) => Vec::from(patterns),
        None => find_gitignore_patterns(path).unwrap_or_default(),
    };

    generate_tree_with_patterns(path, prefix, &patterns, depth)
}

/// Internal function that does the actual tree generation with the provided ignore patterns
fn generate_tree_with_patterns(
    path: &Path,
    prefix: &str,
    ignore: &[Regex],
    depth: Option<usize>,
) -> String {
    if let Some(0) = depth {
        return String::new();
    }
    let mut output = String::new();
    // Panic if the directory cannot be read.
    let entries = fs::read_dir(path).expect("Failed to read directory");

    let mut entries: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let binding = entry.file_name();
            let file_name = binding.to_string_lossy();

            // Create longer-lived bindings to fix temporary value errors
            let entry_path = entry.path();
            let rel_path = match entry_path.strip_prefix(path) {
                Ok(stripped) => stripped,
                Err(_) => &entry_path,
            };

            let rel_path_str = rel_path.to_string_lossy();

            !ignore
                .iter()
                .any(|r| r.is_match(&file_name) || r.is_match(&rel_path_str))
        })
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let len = entries.len();
    for (i, entry) in entries.into_iter().enumerate() {
        let file_name = entry.file_name().into_string().unwrap_or_default();
        let is_last = i == len - 1;
        let connector = if is_last { "└── " } else { "├── " };
        output.push_str(&format!("{prefix}{connector}{file_name}\n"));
        let new_path = entry.path();
        if new_path.is_dir() {
            let new_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}│   ")
            };
            if depth.unwrap_or(usize::MAX) > 0 {
                let new_depth = depth.map(|d| d - 1);
                output.push_str(&generate_tree_with_patterns(
                    &new_path,
                    &new_prefix,
                    ignore,
                    new_depth,
                ));
            }
        }
    }
    output
}

/// Finds the repository root by looking for a .git directory
fn find_repo_root(start_path: &Path) -> Option<PathBuf> {
    let mut current = start_path.to_path_buf();

    loop {
        let git_dir = current.join(".git");
        if (git_dir.exists() && git_dir.is_dir()) || current.join(".gitignore").exists() {
            return Some(current);
        }

        if !current.pop() {
            // We've reached the root of the file system
            return None;
        }
    }
}

/// Collects gitignore patterns from all .gitignore files
fn find_gitignore_patterns(start_path: &Path) -> io::Result<Vec<Regex>> {
    let repo_root = find_repo_root(start_path).unwrap_or_default();

    let mut patterns = Vec::new();

    // First check repository root .gitignore
    let root_gitignore = repo_root.join(".gitignore");
    if root_gitignore.exists() {
        let root_patterns = parse_gitignore(&root_gitignore)?;
        patterns.extend(root_patterns);
    }

    // Optionally, you could recursively find all .gitignore files in the repo
    // But for simplicity, we'll just use the root one

    Ok(patterns)
}

/// Parses a .gitignore file and converts patterns to regexes
fn parse_gitignore(gitignore_path: &Path) -> io::Result<Vec<Regex>> {
    let file = fs::File::open(gitignore_path)?;
    let reader = BufReader::new(file);
    let mut patterns = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Convert .gitignore pattern to regex
        // This is a simplified conversion, a real implementation would be more complex
        let mut pattern_str = String::new();

        // Handle negation (we'll ignore it for simplicity)
        let mut pattern = trimmed;
        if pattern.starts_with('!') {
            pattern = &pattern[1..];
        }

        // Handle directory indicator
        let is_dir = pattern.ends_with('/');
        if is_dir {
            pattern = &pattern[..pattern.len() - 1];
        }

        // Escape regex special characters except * and ?
        for c in pattern.chars() {
            match c {
                '*' => pattern_str.push_str(".*"),
                '?' => pattern_str.push('.'),
                '.' | '+' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                    pattern_str.push('\\');
                    pattern_str.push(c);
                }
                _ => pattern_str.push(c),
            }
        }

        // Make the pattern match the full name
        pattern_str = format!("^{pattern_str}$");

        // Create the regex
        match Regex::new(&pattern_str) {
            Ok(regex) => patterns.push(regex),
            Err(_) => eprintln!("Failed to convert gitignore pattern to regex: {trimmed}"),
        }
    }

    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::Write,
    };

    use regex::Regex;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_generate_tree() {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let base_path = temp_dir.path();

        fs::create_dir(base_path.join("src")).expect("Failed to create directory");
        fs::create_dir(base_path.join("docs")).expect("Failed to create directory");
        fs::create_dir(base_path.join("tests")).expect("Failed to create directory");
        fs::create_dir(base_path.join("src/components")).expect("Failed to create directory");
        fs::create_dir(base_path.join("src/utils")).expect("Failed to create directory");
        fs::create_dir(base_path.join("src/components/common"))
            .expect("Failed to create directory");
        fs::create_dir(base_path.join("docs/api")).expect("Failed to create directory");
        fs::create_dir(base_path.join("tests/unit")).expect("Failed to create directory");
        // Create an empty directory.
        fs::create_dir(base_path.join("empty_dir")).expect("Failed to create directory");

        File::create(base_path.join("README.md")).expect("Failed to create file");
        File::create(base_path.join(".gitignore")).expect("Failed to create file");
        File::create(base_path.join("package.json")).expect("Failed to create file");
        File::create(base_path.join("src/index.ts")).expect("Failed to create file");
        File::create(base_path.join("src/types.d.ts")).expect("Failed to create file");
        File::create(base_path.join("src/components/App.tsx")).expect("Failed to create file");
        File::create(base_path.join("src/components/common/Button.tsx"))
            .expect("Failed to create file");
        File::create(base_path.join("src/components/common/Input.tsx"))
            .expect("Failed to create file");
        File::create(base_path.join("src/utils/helpers.ts")).expect("Failed to create file");
        File::create(base_path.join("docs/api/v1.md")).expect("Failed to create file");
        File::create(base_path.join("docs/api/v2.md")).expect("Failed to create file");
        File::create(base_path.join("tests/unit/helpers.test.ts")).expect("Failed to create file");
        // Create a file with special characters.
        File::create(base_path.join("src/components/Hello World.tsx"))
            .expect("Failed to create file");

        let expected = "\
├── .gitignore
├── README.md
├── docs
│   └── api
│       ├── v1.md
│       └── v2.md
├── empty_dir
├── package.json
├── src
│   ├── components
│   │   ├── App.tsx
│   │   ├── Hello World.tsx
│   │   └── common
│   │       ├── Button.tsx
│   │       └── Input.tsx
│   ├── index.ts
│   ├── types.d.ts
│   └── utils
│       └── helpers.ts
└── tests
    └── unit
        └── helpers.test.ts
";
        let result = generate_tree(base_path, "", None, None);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_tree_ignore() {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let base_path = temp_dir.path();
        // Create files: a.txt, .hidden, b.txt.
        File::create(base_path.join("a.txt")).expect("Failed to create file");
        File::create(base_path.join(".hidden")).expect("Failed to create file");
        File::create(base_path.join("b.txt")).expect("Failed to create file");
        let expected = "\
├── a.txt
└── b.txt
";
        let ignore = [Regex::new(r"^\..*").unwrap()];
        let result = generate_tree(base_path, "", Some(&ignore), None);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_tree_depth_limited() {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let base_path = temp_dir.path();
        // Create a file and a subdirectory at the root.
        File::create(base_path.join("a.txt")).expect("Failed to create file");
        fs::create_dir(base_path.join("subdir")).expect("Failed to create directory");
        File::create(base_path.join("subdir").join("b.txt")).expect("Failed to create file");

        // With depth = Some(1), subdirectory contents should not be shown.
        let expected_depth1 = "\
├── a.txt
└── subdir
";
        let result_depth1 = generate_tree(base_path, "", None, Some(1));
        assert_eq!(result_depth1, expected_depth1);

        // With depth = Some(2), the subdirectory contents are shown.
        let expected_depth2 = "\
├── a.txt
└── subdir
    └── b.txt
";
        let result_depth2 = generate_tree(base_path, "", None, Some(2));
        assert_eq!(result_depth2, expected_depth2);
    }

    #[test]
    fn test_gitignore_integration() {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let base_path = temp_dir.path();

        // Create a .git directory to mark this as a repository root
        fs::create_dir(base_path.join(".git")).expect("Failed to create .git directory");

        // Create a .gitignore file with patterns
        let mut gitignore =
            File::create(base_path.join(".gitignore")).expect("Failed to create .gitignore");
        writeln!(gitignore, "# Node modules").expect("Failed to write to .gitignore");
        writeln!(gitignore, "node_modules/").expect("Failed to write to .gitignore");
        writeln!(gitignore, "*.log").expect("Failed to write to .gitignore");
        writeln!(gitignore, ".DS_Store").expect("Failed to write to .gitignore");
        writeln!(gitignore, "dist").expect("Failed to write to .gitignore");

        // Create directory structure with some ignored files/folders
        fs::create_dir(base_path.join("src")).expect("Failed to create directory");
        fs::create_dir(base_path.join("node_modules")).expect("Failed to create directory");
        fs::create_dir(base_path.join("dist")).expect("Failed to create directory");

        File::create(base_path.join("README.md")).expect("Failed to create file");
        File::create(base_path.join("package.json")).expect("Failed to create file");
        File::create(base_path.join("npm-debug.log")).expect("Failed to create file");
        File::create(base_path.join(".DS_Store")).expect("Failed to create file");
        File::create(base_path.join("src/index.js")).expect("Failed to create file");
        File::create(base_path.join("src/app.log")).expect("Failed to create file");

        // Create a nested file in ignored directory to ensure complete ignoring
        fs::create_dir(base_path.join("node_modules/example")).expect("Failed to create directory");
        File::create(base_path.join("node_modules/example/package.json"))
            .expect("Failed to create file");

        // Expected tree without manually specifying ignore patterns
        let expected = "\
├── .git
├── .gitignore
├── README.md
├── package.json
└── src
    └── index.js
";

        // Call generate_tree without explicitly providing ignore patterns
        // It should automatically use patterns from .gitignore
        let result = generate_tree(base_path, "", None, None);

        // Verify that gitignore patterns were applied
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_repo_root() {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let base_path = temp_dir.path();

        // Initially there should be no repo root
        assert!(find_repo_root(base_path).is_none());

        // Create a .git directory
        fs::create_dir(base_path.join(".git")).expect("Failed to create .git directory");

        // Now we should find the repo root
        let repo_root = find_repo_root(base_path);
        assert!(repo_root.is_some());
        assert_eq!(repo_root.unwrap(), base_path);

        // Create a subdirectory and verify we can find the root from there too
        fs::create_dir(base_path.join("subdir")).expect("Failed to create directory");
        let subdir_path = base_path.join("subdir");

        let repo_root_from_subdir = find_repo_root(&subdir_path);
        assert!(repo_root_from_subdir.is_some());
        assert_eq!(repo_root_from_subdir.unwrap(), base_path);
    }

    #[test]
    fn test_parse_gitignore() {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let gitignore_path = temp_dir.path().join(".gitignore");

        // Create a .gitignore with various pattern styles
        let mut gitignore = File::create(&gitignore_path).expect("Failed to create .gitignore");
        use std::io::Write;
        writeln!(gitignore, "# Comment line").expect("Failed to write to .gitignore");
        writeln!(gitignore, "").expect("Failed to write to .gitignore"); // Empty line
        writeln!(gitignore, "node_modules/").expect("Failed to write to .gitignore");
        writeln!(gitignore, "*.log").expect("Failed to write to .gitignore");
        writeln!(gitignore, "!important.log").expect("Failed to write to .gitignore"); // Negation
        writeln!(gitignore, ".DS_Store").expect("Failed to write to .gitignore");

        // Parse the gitignore file
        let patterns = parse_gitignore(&gitignore_path).expect("Failed to parse .gitignore");

        // Test a few key patterns
        assert_eq!(patterns.len(), 4); // Should have 4 patterns (excluding comments and empty lines)

        // Test that patterns match correctly
        let node_modules_pattern = &patterns[0];
        assert!(node_modules_pattern.is_match("node_modules"));

        let log_pattern = &patterns[1];
        assert!(log_pattern.is_match("debug.log"));
        assert!(log_pattern.is_match("error.log"));
        assert!(!log_pattern.is_match("debug.txt"));

        // Note: We're ignoring negation patterns in our implementation

        let ds_store_pattern = &patterns[3];
        assert!(ds_store_pattern.is_match(".DS_Store"));
        assert!(!ds_store_pattern.is_match("DS_Store"));
    }
}
