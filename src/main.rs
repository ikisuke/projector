use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, exit};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

fn get_developer_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("Developer"))
}

fn get_directories(path: &PathBuf) -> Vec<String> {
    let mut dirs = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if let Some(name) = entry_path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        // 隠しディレクトリをスキップ
                        if !name_str.starts_with('.') {
                            dirs.push(name_str.to_string());
                        }
                    }
                }
            }
        }
    }

    dirs.sort();
    dirs
}

fn start_tmux_session(session_name: &str, project_path: &PathBuf) -> Result<(), String> {
    let session_name = session_name.to_lowercase();
    let path_str = project_path.to_string_lossy();

    // セッションが既に存在するかチェック
    let check = Command::new("tmux")
        .args(["has-session", "-t", &session_name])
        .output();

    if let Ok(output) = check {
        if output.status.success() {
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
        .args(["new-session", "-d", "-s", &session_name, "-c", &path_str])
        .status()
        .map_err(|e| format!("tmux new-session failed: {}", e))?;

    if !status.success() {
        return Err("tmux new-session に失敗しました".to_string());
    }

    // 垂直分割
    let status = Command::new("tmux")
        .args(["split-window", "-h", "-t", &session_name, "-c", &path_str])
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

fn shorten_path(path: &PathBuf) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(relative) = path.strip_prefix(&home) {
            return format!("~/{}", relative.display());
        }
    }
    path.display().to_string()
}

fn render(
    stdout: &mut io::Stdout,
    current_path: &PathBuf,
    items: &[String],
    selected: usize,
) -> io::Result<()> {
    execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    // ヘッダー
    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(format!(" {}\n", shorten_path(current_path))),
        ResetColor,
        Print(" ─────────────────────────────────────\n"),
        SetForegroundColor(Color::DarkGrey),
        Print(" [↑↓] 移動  [Space] 入る  [Enter] TMUX  [←/BS] 戻る  [q] 終了\n"),
        ResetColor,
        Print("\n")
    )?;

    if items.is_empty() {
        execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print("   (サブディレクトリなし)\n"),
            ResetColor
        )?;
    } else {
        for (i, item) in items.iter().enumerate() {
            if i == selected {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(format!(" ❯ {}/\n", item)),
                    ResetColor
                )?;
            } else {
                execute!(stdout, Print(format!("   {}/\n", item)))?;
            }
        }
    }

    stdout.flush()?;
    Ok(())
}

fn run() -> io::Result<()> {
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

    let mut current_path = developer_path.clone();
    let mut path_stack: Vec<PathBuf> = vec![];
    let mut items = get_directories(&current_path);
    let mut selected: usize = 0;

    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let result = (|| -> io::Result<Option<PathBuf>> {
        loop {
            render(&mut stdout, &current_path, &items, selected)?;

            if let Event::Key(key_event) = event::read()? {
                if key_event.kind != KeyEventKind::Press {
                    continue;
                }

                match key_event.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(None);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !items.is_empty() && selected > 0 {
                            selected -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !items.is_empty() && selected < items.len() - 1 {
                            selected += 1;
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Right => {
                        // スペースまたは→: ディレクトリに入る
                        if !items.is_empty() {
                            let new_path = current_path.join(&items[selected]);
                            let new_items = get_directories(&new_path);
                            if !new_items.is_empty() {
                                path_stack.push(current_path.clone());
                                current_path = new_path;
                                items = new_items;
                                selected = 0;
                            }
                        }
                    }
                    KeyCode::Backspace | KeyCode::Left => {
                        // Backspaceまたは←: 親ディレクトリに戻る
                        if let Some(prev_path) = path_stack.pop() {
                            current_path = prev_path;
                            items = get_directories(&current_path);
                            selected = 0;
                        }
                    }
                    KeyCode::Enter => {
                        // Enter: TMUXを起動
                        if !items.is_empty() {
                            let project_path = current_path.join(&items[selected]);
                            return Ok(Some(project_path));
                        }
                    }
                    _ => {}
                }
            }
        }
    })();

    // クリーンアップ
    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    match result {
        Ok(Some(project_path)) => {
            let session_name = project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default");

            println!("選択: {} -> TMUXを起動します...", shorten_path(&project_path));

            if let Err(e) = start_tmux_session(session_name, &project_path) {
                eprintln!("エラー: {}", e);
                exit(1);
            }
        }
        Ok(None) => {
            println!("キャンセルされました");
        }
        Err(e) => {
            eprintln!("エラー: {}", e);
            exit(1);
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("エラー: {}", e);
        exit(1);
    }
}
