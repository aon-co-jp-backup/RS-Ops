use crate::model::ProjectEntry;
use std::fs;
use std::path::Path;
use std::process::Command;

/// `root`直下のディレクトリを1階層だけ走査し、`Cargo.toml`/`package.json`/
/// `.runo-project`のいずれかを持つものをプロジェクトとして検出する
/// (runo-scannerの簡易版。詳細な差分キャッシュ・再帰探索はrunno-scanner
/// 本体に委ね、ここではRS-OpsのUI表示に必要な最小限の実装とする)。
pub fn scan_projects(root: &Path) -> Vec<ProjectEntry> {
    let markers = ["Cargo.toml", "package.json", ".runo-project"];
    let exclude = ["node_modules", ".git", "target", "dist", "build"];
    let mut out = Vec::new();

    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if exclude.contains(&name.as_str()) {
            continue;
        }
        let has_marker = markers.iter().any(|m| path.join(m).exists());
        if !has_marker {
            continue;
        }
        let github_url = git_remote_url(&path);
        let description = extract_description(&path);
        out.push(ProjectEntry {
            name,
            path: path.to_string_lossy().to_string(),
            github_url,
            description,
            priority: None,
        });
    }

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn git_remote_url(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s.trim_end_matches(".git").to_string())
    }
}

/// CLAUDE.md(無ければREADME.md)から一行程度の役割説明を抽出する
/// (runo-scannerの`extract_description`と同じアプローチ)。
fn extract_description(path: &Path) -> Option<String> {
    for file in [path.join("CLAUDE.md"), path.join("README.md")] {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        let mut seen_heading = false;
        for line in lines.iter() {
            let t = line.trim();
            if t.starts_with('#') {
                seen_heading = true;
                continue;
            }
            if seen_heading && !t.is_empty() && !t.starts_with('>') && !t.starts_with('[') {
                let truncated: String = t.chars().take(80).collect();
                return Some(if t.chars().count() > 80 {
                    format!("{truncated}…")
                } else {
                    truncated
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn detects_cargo_project_with_description() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("my-proj");
        fs::create_dir(&proj).unwrap();
        File::create(proj.join("Cargo.toml")).unwrap();
        let mut readme = File::create(proj.join("README.md")).unwrap();
        writeln!(readme, "# My Proj\n\nThis is a test project.").unwrap();

        let found = scan_projects(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "my-proj");
        assert_eq!(found[0].description.as_deref(), Some("This is a test project."));
    }

    #[test]
    fn ignores_dirs_without_markers() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("not-a-project")).unwrap();
        let found = scan_projects(dir.path());
        assert!(found.is_empty());
    }

    #[test]
    fn ignores_excluded_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let node_modules = dir.path().join("node_modules");
        fs::create_dir(&node_modules).unwrap();
        File::create(node_modules.join("Cargo.toml")).unwrap();
        let found = scan_projects(dir.path());
        assert!(found.is_empty());
    }
}
