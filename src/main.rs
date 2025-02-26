use std::env;
use std::path::Path;
use serde_json::json;

mod tree;
mod claude_client;
use claude_client::{ClaudeClient, ClaudeRequest};

fn main() {
    // ここでは CLI の引数などからコードベースのパスを取得する例
    let args: Vec<String> = env::args().collect();
    let base_path = if args.len() > 1 { &args[1] } else { "." };
    let path = Path::new(base_path);

    // tree 機能の実行
    match tree::generate_tree(path, "") {
        Ok(result) => {
            println!("ディレクトリ構造:\n{}", result);

            // マスターエージェントとしてclaudeクライアントにリクエストを送る例
            let claude_client = ClaudeClient::new("https://api.claude.ai", "YOUR_API_KEY");
            let request = ClaudeRequest {
                sender: "master".to_string(),
                receiver: "claude".to_string(),
                message_type: "task_result".to_string(),
                task_id: "tree-task-001".to_string(),
                command: "tree".to_string(),
                payload: json!({
                    "result": result
                }),
            };

            match claude_client.send_request(&request) {
                Ok(response) => println!("Claude Response: status={}, payload={:?}", response.status, response.payload),
                Err(e) => eprintln!("Claude からの応答に失敗: {}", e),
            }
        }
        Err(e) => eprintln!("tree 実行エラー: {}", e),
    }
}