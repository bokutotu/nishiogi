use std::{fs, io, path::Path};

use regex::Regex;

pub fn generate_tree(
    path: &Path,
    prefix: &str,
    ignore: Option<&[Regex]>,
    depth: Option<usize>,
) -> io::Result<String> {
    if let Some(0) = depth {
        return Ok(String::new());
    }
    let mut output = String::new();
    let entries = fs::read_dir(path)?;

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
        output.push_str(&format!("{prefix}{connector}{file_name}\n",));
        let new_path = entry.path();
        if new_path.is_dir() {
            let new_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}│   ")
            };
            if depth.unwrap_or(usize::MAX) > 0 {
                let new_depth = depth.map(|d| d - 1);
                output.push_str(&generate_tree(&new_path, &new_prefix, ignore, new_depth)?);
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::fs::{self, File};
    use tempfile::TempDir;

    #[test]
    fn test_generate_tree() -> io::Result<()> {
        // テスト用の一時ディレクトリを作成
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path();

        // より複雑なディレクトリ構造を作成
        fs::create_dir(base_path.join("src"))?;
        fs::create_dir(base_path.join("docs"))?;
        fs::create_dir(base_path.join("tests"))?;
        fs::create_dir(base_path.join("src/components"))?;
        fs::create_dir(base_path.join("src/utils"))?;
        fs::create_dir(base_path.join("src/components/common"))?;
        fs::create_dir(base_path.join("docs/api"))?;
        fs::create_dir(base_path.join("tests/unit"))?;
        // 空のディレクトリ
        fs::create_dir(base_path.join("empty_dir"))?;

        // ファイルを作成
        File::create(base_path.join("README.md"))?;
        File::create(base_path.join(".gitignore"))?;
        File::create(base_path.join("package.json"))?;
        File::create(base_path.join("src/index.ts"))?;
        File::create(base_path.join("src/types.d.ts"))?;
        File::create(base_path.join("src/components/App.tsx"))?;
        File::create(base_path.join("src/components/common/Button.tsx"))?;
        File::create(base_path.join("src/components/common/Input.tsx"))?;
        File::create(base_path.join("src/utils/helpers.ts"))?;
        File::create(base_path.join("docs/api/v1.md"))?;
        File::create(base_path.join("docs/api/v2.md"))?;
        File::create(base_path.join("tests/unit/helpers.test.ts"))?;
        // 特殊文字を含むファイル
        File::create(base_path.join("src/components/Hello World.tsx"))?;

        // 期待される出力
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
        // 実際の出力を取得
        let result = generate_tree(base_path, "", None, None)?;

        // 結果を比較
        assert_eq!(result, expected);
        Ok(())
    }

    #[test]
    fn test_generate_tree_ignore() -> io::Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path();
        // 作成するファイル： a.txt, .hidden, b.txt
        File::create(base_path.join("a.txt"))?;
        File::create(base_path.join(".hidden"))?;
        File::create(base_path.join("b.txt"))?;
        let expected = "\
├── a.txt
└── b.txt
";
        let ignore = [Regex::new(r"^\..*").unwrap()];
        let result = generate_tree(base_path, "", Some(&ignore), None)?;
        assert_eq!(result, expected);
        Ok(())
    }

    #[test]
    fn test_generate_tree_depth_limited() -> io::Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path();
        // ルートにファイルとディレクトリを作成
        File::create(base_path.join("a.txt"))?;
        fs::create_dir(base_path.join("subdir"))?;
        File::create(base_path.join("subdir").join("b.txt"))?;

        // depth = Some(1) の場合、サブディレクトリ内の内容は表示されず、サブディレクトリ名のみが出力される
        let expected_depth1 = "\
├── a.txt
└── subdir
";
        let result_depth1 = generate_tree(base_path, "", None, Some(1))?;
        assert_eq!(result_depth1, expected_depth1);

        // depth = Some(2) の場合、サブディレクトリ内も表示される
        let expected_depth2 = "\
├── a.txt
└── subdir
    └── b.txt
";
        let result_depth2 = generate_tree(base_path, "", None, Some(2))?;
        assert_eq!(result_depth2, expected_depth2);

        Ok(())
    }
}
