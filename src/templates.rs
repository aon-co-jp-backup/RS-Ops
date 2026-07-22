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

/// 課題管理システム(RS-Chiketto/Redmine)のチケット1件を、対象AIツール
/// へそのまま渡せるタスク依頼プロンプトに変換する。
/// (`ticket_automation`モジュールから、新規チケット検出時に呼ばれる)。
pub fn generate_ticket_task_prompt(
    target: AiTarget,
    ticket_id: u64,
    title: &str,
    description: &str,
    tracker_url: &str,
) -> String {
    match target {
        AiTarget::Claude => format!(
            "# 課題対応依頼(チケット #{ticket_id})\n\n\
             ## タイトル\n{title}\n\n\
             ## 内容\n{description}\n\n\
             ## 参照元\n{tracker_url}\n\n\
             このチケットの内容を確認し、実装・修正を行ってください。\
             完了したら実ビルド・実テストで検証し、チケットのステータス更新も検討してください。\
             型チェックのみで「完了」と報告しないこと。\n"
        ),
        AiTarget::Cursor => format!(
            "# Ticket #{ticket_id}: {title}\n\n\
             {description}\n\n\
             Source: {tracker_url}\n\n\
             Please implement/fix this. Verify with a real build/test before considering it done.\n"
        ),
        AiTarget::Generic => format!(
            "You have been assigned ticket #{ticket_id} from an issue tracker.\n\n\
             Title: {title}\n\
             Description: {description}\n\
             Source: {tracker_url}\n\n\
             Please implement or fix what this ticket describes, and verify your change with \
             an actual build/test run — do not report completion based on a type-check alone.\n"
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

    #[test]
    fn generates_ticket_prompt_for_each_target() {
        for target in [AiTarget::Claude, AiTarget::Cursor, AiTarget::Generic] {
            let out = generate_ticket_task_prompt(target, 42, "Fix login bug", "Users can't log in.", "https://runo.tokyo/chiketto");
            assert!(out.contains("42"));
            assert!(out.contains("Fix login bug"));
            assert!(out.contains("Users can't log in."));
        }
    }
}
