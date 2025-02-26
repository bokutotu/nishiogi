use std::env;
use std::path::Path;
use serde_json::json;
use chrono::Utc;

mod tree;
mod claude_client;
use claude_client::{ClaudeClient, ClaudeRequest};

fn main() {
    // API KEYを環境変数から取得
    let api_key = env::var("CLAUDE_API_KEY").unwrap_or_else(|_| {
        eprintln!("警告: CLAUDE_API_KEYが設定されていません");
        "YOUR_API_KEY".to_string()
    });

    // コマンドライン引数からベースパスを取得
    let args: Vec<String> = env::args().collect();
    let base_path = if args.len() > 1 { &args[1] } else { "." };
    let path = Path::new(base_path);

    // マスターエージェントとしてtreeタスクを実行
    let task_id = format!("tree-task-{}", Utc::now().timestamp());
    match tree::generate_tree(path, "") {
        Ok(result) => {
            println!("ディレクトリ構造解析完了:\n{}", result);

            // Claudeクライアントを初期化してtree結果を送信
            let claude_client = ClaudeClient::new("https://api.claude.ai", &api_key);
            let request = ClaudeRequest {
                sender: "master".to_string(),
                receiver: "claude".to_string(),
                message_type: "task_result".to_string(),
                task_id: task_id,
                command: "tree".to_string(),
                payload: json!({
                    "results": {
                        "directory_structure": result
                    }
                }),
                status: Some("completed".to_string()),
                metadata: Some(json!({
                    "priority": 1,
                    "retry_count": 0
                })),
            };

            match claude_client.send_request(&request) {
                Ok(response) => println!("解析結果送信完了: status={}, payload={:?}", response.status, response.payload),
                Err(e) => eprintln!("解析結果送信エラー: {}", e),
            }
        }
        Err(e) => eprintln!("ディレクトリ構造解析エラー: {}", e),
    }
}