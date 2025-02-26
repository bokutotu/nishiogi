//! # Directory Tree Generator
//!
//! This module provides a function to generate a textual representation of a directory tree.
//! It recursively traverses a given directory, allowing you to ignore files or directories
//! that match provided regular expressions, and optionally limits the depth of the tree.
//!
//! **Note:** This function will panic if it fails to read a directory (for example, due to
//! insufficient permissions or a non-existent path).
//!
//! ## Example
//!
//! ```rust
//! use std::path::Path;
//! use regex::Regex;
//! use tree_generator::generate_tree;
//!
//! # fn main() {
//! let path = Path::new(".");
//! let ignore_patterns = [Regex::new(r"^\..*").unwrap()]; // ignore hidden files
//! let tree = generate_tree(path, "", Some(&ignore_patterns), Some(3));
//! println!("{}", tree);
//! # }
//! ```

use regex::Regex;
use std::{fs, path::Path};

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
///
/// # Examples
///
/// ```rust
/// use std::path::Path;
/// use regex::Regex;
/// use tree_generator::generate_tree;
///
/// # fn main() {
/// let path = Path::new(".");
/// let ignore_patterns = [Regex::new(r"^\..*").unwrap()]; // ignore hidden files
/// let tree = generate_tree(path, "", Some(&ignore_patterns), Some(3));
/// println!("{}", tree);
/// # }
/// ```
pub fn generate_tree(
    path: &Path,
    prefix: &str,
    ignore: Option<&[Regex]>,
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
            !ignore
                .unwrap_or_default()
                .iter()
                .any(|r| r.is_match(&file_name))
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
            let new_prefix = if prefix.is_empty() {
                if is_last {
                    format!("{prefix}    ")
                } else {
                    format!("{prefix}│   ")
                }
            } else {
                format!("{prefix}    ")
            };
            if depth.unwrap_or(usize::MAX) > 0 {
                let new_depth = depth.map(|d| d - 1);
                output.push_str(&generate_tree(&new_path, &new_prefix, ignore, new_depth));
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::fs::{self, File};
    use tempfile::TempDir;

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
│       ├── App.tsx
│       ├── Hello World.tsx
│       └── common
│           ├── Button.tsx
│           └── Input.tsx
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
}
