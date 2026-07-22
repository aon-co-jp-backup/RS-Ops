use crate::model::AiTarget;

/// プロジェクト名・説明・GitHub URLから、対象AIツール向けの
/// コンテキストファイル雛形を生成する。既存プロジェクトの
/// `CLAUDE.md`等を上書きする機能ではなく、**新規プロジェクト向けの
/// 雛形生成**という位置づけ(既存の丁寧な手書き記述を壊さないため)。
pub fn generate_template(target: AiTarget, project_name: &str, description: &str, repo_url: &str) -> String {
    match target {
        AiTarget::Claude => format!(
            "# 開発方針・開発環境ルール({project_name})\n\n\
             ## このプロジェクトの役割\n\n\
             {description}\n\n\
             GitHubリポジトリ: {repo_url}\n\n\
             ## HANDOFF\n\n\
             - (この節に、セッションごとの作業ログを追記していく)\n"
        ),
        AiTarget::Cursor => format!(
            "# {project_name}\n\n\
             {description}\n\n\
             Repository: {repo_url}\n\n\
             - Prefer small, verifiable changes; run the test suite before considering a change complete.\n\
             - Do not fabricate completion — verify with a real build/test/HTTP check.\n"
        ),
        AiTarget::Generic => format!(
            "# AGENTS.md — {project_name}\n\n\
             You are assisting with development on the \"{project_name}\" project.\n\n\
             ## Project role\n{description}\n\n\
             ## Repository\n{repo_url}\n\n\
             ## Ground rules\n\
             - Verify claims with actual build/test/HTTP checks before reporting completion.\n\
             - Prefer small, reversible changes.\n\
             (This generic template is intended as a system-prompt seed for ChatGPT, Gemini, DeepSeek, Grok, or any other LLM-based coding assistant.)\n"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_claude_md_with_project_name() {
        let out = generate_template(AiTarget::Claude, "demo-proj", "A demo.", "https://github.com/x/demo-proj");
        assert!(out.contains("demo-proj"));
        assert!(out.contains("A demo."));
    }

    #[test]
    fn generates_cursorrules() {
        let out = generate_template(AiTarget::Cursor, "demo-proj", "A demo.", "https://github.com/x/demo-proj");
        assert!(out.contains("demo-proj"));
    }

    #[test]
    fn generates_generic_agents_md() {
        let out = generate_template(AiTarget::Generic, "demo-proj", "A demo.", "https://github.com/x/demo-proj");
        assert!(out.contains("AGENTS.md"));
    }
}
