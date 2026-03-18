//! Language detection from workspace files

use std::path::Path;

/// Detect programming languages based on project files in the workspace root.
///
/// This only checks the root directory (non-recursive) for known project files.
pub fn detect(workspace: &Path) -> Vec<String> {
    let mut languages = Vec::new();

    // Rust: Cargo.toml
    if workspace.join("Cargo.toml").exists() {
        languages.push("rust".to_string());
    }

    // JavaScript/TypeScript: package.json
    if workspace.join("package.json").exists() {
        // Check for TypeScript config
        if workspace.join("tsconfig.json").exists() {
            languages.push("typescript".to_string());
        } else {
            languages.push("javascript".to_string());
        }
    }

    // Go: go.mod
    if workspace.join("go.mod").exists() {
        languages.push("go".to_string());
    }

    // Python: pyproject.toml or setup.py or requirements.txt
    if workspace.join("pyproject.toml").exists()
        || workspace.join("setup.py").exists()
        || workspace.join("requirements.txt").exists()
    {
        languages.push("python".to_string());
    }

    languages
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_temp_workspace(files: &[&str]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for file in files {
            fs::write(dir.path().join(file), "").unwrap();
        }
        dir
    }

    #[test]
    fn test_detect_rust() {
        let dir = create_temp_workspace(&["Cargo.toml"]);
        let langs = detect(dir.path());
        assert_eq!(langs, vec!["rust"]);
    }

    #[test]
    fn test_detect_javascript() {
        let dir = create_temp_workspace(&["package.json"]);
        let langs = detect(dir.path());
        assert_eq!(langs, vec!["javascript"]);
    }

    #[test]
    fn test_detect_typescript() {
        let dir = create_temp_workspace(&["package.json", "tsconfig.json"]);
        let langs = detect(dir.path());
        assert_eq!(langs, vec!["typescript"]);
    }

    #[test]
    fn test_detect_go() {
        let dir = create_temp_workspace(&["go.mod"]);
        let langs = detect(dir.path());
        assert_eq!(langs, vec!["go"]);
    }

    #[test]
    fn test_detect_python() {
        let dir = create_temp_workspace(&["pyproject.toml"]);
        let langs = detect(dir.path());
        assert_eq!(langs, vec!["python"]);
    }

    #[test]
    fn test_detect_none() {
        let dir = TempDir::new().unwrap();
        let langs = detect(dir.path());
        assert!(langs.is_empty());
    }

    #[test]
    fn test_detect_multiple() {
        let dir = create_temp_workspace(&["Cargo.toml", "package.json"]);
        let langs = detect(dir.path());
        assert_eq!(langs, vec!["rust", "javascript"]);
    }
}
