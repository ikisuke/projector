use std::fs;
use std::process::{Command, exit};
use std::path::PathBuf;

use console::Term;
use dialoguer::{Select, theme::ColorfulTheme};

fn get_developer_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("Developer"))
}

fn get_projects(developer_path: &PathBuf) -> Vec<String> {
    let mut projects = Vec::new();

    if let Ok(entries) = fs::read_dir(developer_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        projects.push(name_str.to_string());
                    }
                }
            }
        }
    }

    projects.sort();
    projects
}

fn start_tmux_session(project_name: &str, project_path: &PathBuf) -> Result<(), String> {
    let session_name = project_name.to_lowercase();
    let path_str = project_path.to_string_lossy();

    // セッションが既に存在するかチェック
    let check = Command::new("tmux")
        .args(["has-session", "-t", &session_name])
        .output();

    if let Ok(output) = check {
        if output.status.success() {
            // 既存セッションにアタッチ
            println!("セッション '{}' は既に存在します。アタッチします...", session_name);
            let status = Command::new("tmux")
                .args(["attach-session", "-t", &session_name])
                .status()
                .map_err(|e| format!("tmux attach failed: {}", e))?;

            if !status.success() {
                return Err("tmux attach-session に失敗しました".to_string());
            }
            return Ok(());
        }
    }

    // 新規セッションをバックグラウンドで作成
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s", &session_name,
            "-c", &path_str,
        ])
        .status()
        .map_err(|e| format!("tmux new-session failed: {}", e))?;

    if !status.success() {
        return Err("tmux new-session に失敗しました".to_string());
    }

    // 垂直分割（-h は horizontal split = 画面を左右に分割 = 垂直線で分割）
    let status = Command::new("tmux")
        .args([
            "split-window",
            "-h",
            "-t", &session_name,
            "-c", &path_str,
        ])
        .status()
        .map_err(|e| format!("tmux split-window failed: {}", e))?;

    if !status.success() {
        return Err("tmux split-window に失敗しました".to_string());
    }

    // セッションにアタッチ
    let status = Command::new("tmux")
        .args(["attach-session", "-t", &session_name])
        .status()
        .map_err(|e| format!("tmux attach failed: {}", e))?;

    if !status.success() {
        return Err("tmux attach-session に失敗しました".to_string());
    }

    Ok(())
}

fn main() {
    let developer_path = match get_developer_path() {
        Some(path) => path,
        None => {
            eprintln!("ホームディレクトリを取得できませんでした");
            exit(1);
        }
    };

    if !developer_path.exists() {
        eprintln!("~/Developer ディレクトリが存在しません");
        exit(1);
    }

    let projects = get_projects(&developer_path);

    if projects.is_empty() {
        eprintln!("~/Developer にプロジェクトが見つかりません");
        exit(1);
    }

    let term = Term::stderr();
    let theme = ColorfulTheme::default();

    let selection = Select::with_theme(&theme)
        .with_prompt("プロジェクトを選択してください")
        .items(&projects)
        .default(0)
        .interact_on(&term);

    match selection {
        Ok(index) => {
            let project_name = &projects[index];
            let project_path = developer_path.join(project_name);

            println!("選択: {} -> TMUXを起動します...", project_name);

            if let Err(e) = start_tmux_session(project_name, &project_path) {
                eprintln!("エラー: {}", e);
                exit(1);
            }
        }
        Err(_) => {
            println!("キャンセルされました");
            exit(0);
        }
    }
}
