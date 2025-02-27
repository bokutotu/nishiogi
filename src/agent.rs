use crate::github_copilot_client::{CopilotClient, CopilotError, Message};
use crate::tree::generate_tree;
use crate::show_file::read_file_content;
use std::error::Error;
use std::path::Path;

/// Agent のエラー型
#[derive(Debug)]
pub enum AgentError {
    CopilotError(CopilotError),
    ModelNotFound,
    CommandError(String),
    IoError(std::io::Error),
    Other(String),
}

impl From<CopilotError> for AgentError {
    fn from(e: CopilotError) -> Self {
        Self::CopilotError(e)
    }
}

impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<Box<dyn Error>> for AgentError {
    fn from(e: Box<dyn Error>) -> Self {
        Self::Other(e.to_string())
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::CopilotError(e) => write!(f, "Copilot error: {}", e),
            AgentError::ModelNotFound => write!(f, "Model not found"),
            AgentError::CommandError(s) => write!(f, "Command error: {}", s),
            AgentError::IoError(e) => write!(f, "I/O error: {}", e),
            AgentError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl Error for AgentError {}

/// Agent の状態
#[derive(Debug, Default)]
pub struct AgentContext {
    /// 元の質問
    pub question: String,
    /// 計画
    pub plan: Vec<String>,
    /// コマンド実行結果
    pub command_results: Vec<(String, String)>,
    /// 現在の回答案
    pub current_answer: Option<String>,
    /// レビュー結果
    pub review_result: Option<String>,
    /// 現在のステップ
    pub current_step: AgentStep,
}

/// Agent のステップ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentStep {
    #[default]
    /// 質問を解析
    UnderstandQuestion,
    /// 計画を立てる
    PlanExecution,
    /// コマンドを実行
    ExecuteCommands,
    /// 回答を作成
    CreateAnswer,
    /// 回答をレビュー
    ReviewAnswer,
    /// 完了
    Done,
}

/// Agent
pub struct Agent {
    client: CopilotClient,
    model_id: String,
    context: AgentContext,
}

impl Agent {
    /// 新しい Agent を作成
    pub async fn new(editor_version: String, model_id: String) -> Result<Self, AgentError> {
        let client = CopilotClient::from_env_with_models(editor_version).await?;
        Ok(Self {
            client,
            model_id,
            context: AgentContext::default(),
        })
    }

    /// 質問を処理
    pub async fn process_question(&mut self, question: String) -> Result<String, AgentError> {
        self.context = AgentContext::default();
        self.context.question = question;
        
        loop {
            match self.context.current_step {
                AgentStep::UnderstandQuestion => {
                    self.understand_question().await?;
                    self.context.current_step = AgentStep::PlanExecution;
                }
                AgentStep::PlanExecution => {
                    self.plan_execution().await?;
                    self.context.current_step = AgentStep::ExecuteCommands;
                }
                AgentStep::ExecuteCommands => {
                    self.execute_commands().await?;
                    self.context.current_step = AgentStep::CreateAnswer;
                }
                AgentStep::CreateAnswer => {
                    self.create_answer().await?;
                    self.context.current_step = AgentStep::ReviewAnswer;
                }
                AgentStep::ReviewAnswer => {
                    let review_passed = self.review_answer().await?;
                    if review_passed {
                        self.context.current_step = AgentStep::Done;
                    } else {
                        // レビューが通らなかった場合、計画作成から再開
                        self.context.current_step = AgentStep::PlanExecution;
                    }
                }
                AgentStep::Done => {
                    return Ok(self.context.current_answer.clone().unwrap_or_default());
                }
            }
        }
    }

    /// 質問を理解する
    async fn understand_question(&mut self) -> Result<(), AgentError> {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "あなたはコードリポジトリを解析するエージェントです。ユーザーの質問を理解し、その意図を明確にしてください。".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!("以下の質問の意図を分析してください: {}", self.context.question),
            },
        ];

        let response = self.client.chat_completion(messages, self.model_id.clone()).await?;
        if let Some(choice) = response.choices.first() {
            println!("質問の理解: {}", choice.message.content);
        }
        
        Ok(())
    }

    /// 計画を立てる
    async fn plan_execution(&mut self) -> Result<(), AgentError> {
        let mut previous_context = String::new();
        if let Some(review) = &self.context.review_result {
            previous_context = format!(
                "前回の回答に対するレビュー結果: {}\n\n",
                review
            );
        }
        
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "あなたはコードリポジトリを解析するエージェントです。使用可能なコマンドは 'tree <path>' と 'show_file <filepath>' のみです。質問に回答するために必要なコマンドを順番に「コマンド: 」形式でリストアップしてください。".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "{}以下の質問に回答するための計画を立ててください: {}", 
                    previous_context,
                    self.context.question
                ),
            },
        ];

        let response = self.client.chat_completion(messages, self.model_id.clone()).await?;
        if let Some(choice) = response.choices.first() {
            println!("実行計画: {}", choice.message.content);
            
            // 「コマンド: 」で始まる行を抽出
            self.context.plan = choice.message.content
                .lines()
                .filter(|line| line.contains("コマンド:") || line.contains("command:"))
                .map(|line| {
                    let parts: Vec<&str> = line.split(":").collect();
                    if parts.len() > 1 {
                        parts[1..].join(":").trim().to_string()
                    } else {
                        line.trim().to_string()
                    }
                })
                .collect();
                
            // 計画が空の場合、全文を1つのコマンドとして扱う
            if self.context.plan.is_empty() {
                self.context.plan = vec![choice.message.content.trim().to_string()];
            }
        }
        
        Ok(())
    }

    /// コマンドを実行
    async fn execute_commands(&mut self) -> Result<(), AgentError> {
        self.context.command_results.clear();
        
        // 計画から抽出したコマンドを実行
        for command in &self.context.plan {
            let cmd_result = if command.starts_with("tree ") {
                let path = command.strip_prefix("tree ").unwrap_or(".");
                self.run_tree_command(path)?
            } else if command.starts_with("show_file ") {
                let path = command.strip_prefix("show_file ").unwrap_// filepath: /Users/kondouakira/Code/nishiogi/src/agent.rs
use crate::github_copilot_client::{CopilotClient, CopilotError, Message};
use crate::tree::generate_tree;
use crate::show_file::read_file_content;
use std::error::Error;
use std::path::Path;

/// Agent のエラー型
#[derive(Debug)]
pub enum AgentError {
    CopilotError(CopilotError),
    ModelNotFound,
    CommandError(String),
    IoError(std::io::Error),
    Other(String),
}

impl From<CopilotError> for AgentError {
    fn from(e: CopilotError) -> Self {
        Self::CopilotError(e)
    }
}

impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<Box<dyn Error>> for AgentError {
    fn from(e: Box<dyn Error>) -> Self {
        Self::Other(e.to_string())
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::CopilotError(e) => write!(f, "Copilot error: {}", e),
            AgentError::ModelNotFound => write!(f, "Model not found"),
            AgentError::CommandError(s) => write!(f, "Command error: {}", s),
            AgentError::IoError(e) => write!(f, "I/O error: {}", e),
            AgentError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl Error for AgentError {}

/// Agent の状態
#[derive(Debug, Default)]
pub struct AgentContext {
    /// 元の質問
    pub question: String,
    /// 計画
    pub plan: Vec<String>,
    /// コマンド実行結果
    pub command_results: Vec<(String, String)>,
    /// 現在の回答案
    pub current_answer: Option<String>,
    /// レビュー結果
    pub review_result: Option<String>,
    /// 現在のステップ
    pub current_step: AgentStep,
}

/// Agent のステップ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentStep {
    #[default]
    /// 質問を解析
    UnderstandQuestion,
    /// 計画を立てる
    PlanExecution,
    /// コマンドを実行
    ExecuteCommands,
    /// 回答を作成
    CreateAnswer,
    /// 回答をレビュー
    ReviewAnswer,
    /// 完了
    Done,
}

/// Agent
pub struct Agent {
    client: CopilotClient,
    model_id: String,
    context: AgentContext,
}

impl Agent {
    /// 新しい Agent を作成
    pub async fn new(editor_version: String, model_id: String) -> Result<Self, AgentError> {
        let client = CopilotClient::from_env_with_models(editor_version).await?;
        Ok(Self {
            client,
            model_id,
            context: AgentContext::default(),
        })
    }

    /// 質問を処理
    pub async fn process_question(&mut self, question: String) -> Result<String, AgentError> {
        self.context = AgentContext::default();
        self.context.question = question;
        
        loop {
            match self.context.current_step {
                AgentStep::UnderstandQuestion => {
                    self.understand_question().await?;
                    self.context.current_step = AgentStep::PlanExecution;
                }
                AgentStep::PlanExecution => {
                    self.plan_execution().await?;
                    self.context.current_step = AgentStep::ExecuteCommands;
                }
                AgentStep::ExecuteCommands => {
                    self.execute_commands().await?;
                    self.context.current_step = AgentStep::CreateAnswer;
                }
                AgentStep::CreateAnswer => {
                    self.create_answer().await?;
                    self.context.current_step = AgentStep::ReviewAnswer;
                }
                AgentStep::ReviewAnswer => {
                    let review_passed = self.review_answer().await?;
                    if review_passed {
                        self.context.current_step = AgentStep::Done;
                    } else {
                        // レビューが通らなかった場合、計画作成から再開
                        self.context.current_step = AgentStep::PlanExecution;
                    }
                }
                AgentStep::Done => {
                    return Ok(self.context.current_answer.clone().unwrap_or_default());
                }
            }
        }
    }

    /// 質問を理解する
    async fn understand_question(&mut self) -> Result<(), AgentError> {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "あなたはコードリポジトリを解析するエージェントです。ユーザーの質問を理解し、その意図を明確にしてください。".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!("以下の質問の意図を分析してください: {}", self.context.question),
            },
        ];

        let response = self.client.chat_completion(messages, self.model_id.clone()).await?;
        if let Some(choice) = response.choices.first() {
            println!("質問の理解: {}", choice.message.content);
        }
        
        Ok(())
    }

    /// 計画を立てる
    async fn plan_execution(&mut self) -> Result<(), AgentError> {
        let mut previous_context = String::new();
        if let Some(review) = &self.context.review_result {
            previous_context = format!(
                "前回の回答に対するレビュー結果: {}\n\n",
                review
            );
        }
        
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "あなたはコードリポジトリを解析するエージェントです。使用可能なコマンドは 'tree <path>' と 'show_file <filepath>' のみです。質問に回答するために必要なコマンドを順番に「コマンド: 」形式でリストアップしてください。".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "{}以下の質問に回答するための計画を立ててください: {}", 
                    previous_context,
                    self.context.question
                ),
            },
        ];

        let response = self.client.chat_completion(messages, self.model_id.clone()).await?;
        if let Some(choice) = response.choices.first() {
            println!("実行計画: {}", choice.message.content);
            
            // 「コマンド: 」で始まる行を抽出
            self.context.plan = choice.message.content
                .lines()
                .filter(|line| line.contains("コマンド:") || line.contains("command:"))
                .map(|line| {
                    let parts: Vec<&str> = line.split(":").collect();
                    if parts.len() > 1 {
                        parts[1..].join(":").trim().to_string()
                    } else {
                        line.trim().to_string()
                    }
                })
                .collect();
                
            // 計画が空の場合、全文を1つのコマンドとして扱う
            if self.context.plan.is_empty() {
                self.context.plan = vec![choice.message.content.trim().to_string()];
            }
        }
        
        Ok(())
    }

    /// コマンドを実行
    async fn execute_commands(&mut self) -> Result<(), AgentError> {
        self.context.command_results.clear();
        
        // 計画から抽出したコマンドを実行
        for command in &self.context.plan {
            let cmd_result = if command.starts_with("tree ") {
                let path = command.strip_prefix("tree ").unwrap_or(".");
                self.run_tree_command(path)?
            } else if command.starts_with("show_file ") {
                let path = command.strip_prefix("show_file ").unwrap_or("");
                self.run_show_file_command(path)?
            } else {
                return Err(AgentError::CommandError(format!("Unknown command: {}", command)));
            };

            println!("実行結果 ({}): {}", command, &cmd_result[..std::cmp::min(100, cmd_result.len())]);
            self.context.command_results.push((command.clone(), cmd_result));
        }
        
        Ok(())
    }

    /// tree コマンドを実行
    fn run_tree_command(&self, path: &str) -> Result<String, AgentError> {
        let path = Path::new(path);
        Ok(generate_tree(path, "", None, None))
    }

    /// show_file コマンドを実行
    fn run_show_file_command(&self, path: &str) -> Result<String, AgentError> {
        let path = Path::new(path);
        match read_file_content(path) {
            Ok(content) => Ok(content),
            Err(e) => Err(e.into()),
        }
    }

    /// 回答を作成
    async fn create_answer(&mut self) -> Result<(), AgentError> {
        let mut command_results_text = String::new();
        for (cmd, result) in &self.context.command_results {
            command_results_text.push_str(&format!("## コマンド: {}\n\n```\n{}\n```\n\n", cmd, result));
        }
        
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "あなたはコードリポジトリを解析するエージェントです。実行したコマンドの結果に基づいて、ユーザーの質問に回答してください。".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "質問: {}\n\n実行したコマンドの結果:\n\n{}\n\n上記情報に基づいて質問に回答してください。",
                    self.context.question,
                    command_results_text
                ),
            },
        ];

        let response = self.client.chat_completion(messages, self.model_id.clone()).await?;
        if let Some(choice) = response.choices.first() {
            self.context.current_answer = Some(choice.message.content.clone());
            println!("回答案: {}", choice.message.content);
        } else {
            return Err(AgentError::Other("Failed to get answer response".to_string()));
        }
        
        Ok(())
    }

    /// 回答をレビュー
    async fn review_answer(&mut self) -> Result<bool, AgentError> {
        let answer = match &self.context.current_answer {
            Some(a) => a,
            None => return Err(AgentError::Other("No answer to review".to_string())),
        };
        
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "あなたはコードリポジトリを解析するエージェントです。作成した回答が質問に対して適切か評価してください。".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "質問: {}\n\n回答: {}\n\nこの回答は質問に対して適切かつ十分ですか？問題点があれば指摘し、さらなる情報収集が必要なら「不十分」と結論づけてください。適切な場合は「適切」と結論づけてください。",
                    self.context.question,
                    answer
                ),
            },
        ];

        let response = self.client.chat_completion(messages, self.model_id.clone()).await?;
        if let Some(choice) = response.choices.first() {
            let review = choice.message.content.clone();
            self.context.review_result = Some(review.clone());
            println!("レビュー結果: {}", review);
            
            // レビュー結果に「適切」が含まれていればOK
            let passed = review.to_lowercase().contains("適切") && 
                        !review.to_lowercase().contains("不十分") &&
                        !review.to_lowercase().contains("不適切");
            
            Ok(passed)
        } else {
            Err(AgentError::Other("Failed to get review response".to_string()))
        }
    }
}