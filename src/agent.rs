//! # Agent Module
//!
//! This module provides a runtime agent capable of understanding user queries,
//! planning and executing commands, and generating answers based on the results.
//!
//! ## Architecture
//!
//! The Agent follows a six-step workflow:
//!
//! 1. **Intent Extraction**: Analyze user's question to determine what they're asking
//! 2. **Planning**: Create a plan of action to answer the question
//! 3. **Command Execution**: Run commands (currently supports `tree` and `show_file`)
//! 4. **Answer Generation**: Create an answer based on command results
//! 5. **Review**: Evaluate if the answer adequately addresses the question
//! 6. **Iteration**: If review is unsuccessful, repeat the process; otherwise return the answer
//!
//! ## Error Handling
//!
//! The agent implements comprehensive error handling through the `AgentError` enum,
//! allowing for graceful recovery and detailed error reporting.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    github_copilot_client::{CopilotClient, CopilotError, Message},
    show_file::{self, read_file_content, FileReadError},
    tree::generate_tree,
};

const MAX_ITERATIONS: usize = 3;

/// Errors that can occur during agent operations
#[derive(Debug)]
pub enum AgentError {
    // Intent errors
    IntentExtractionFailed,
    MalformedIntentResponse,
    EmptyIntentResponse,

    // Planning errors
    PlanningFailed,
    EmptyPlan,
    InvalidPlanFormat,

    // Command errors
    UnknownCommand(String), // Keep string for command name
    PathNotFound(PathBuf),  // Use PathBuf instead of String
    PathIsDirectory(PathBuf),
    CommandExecutionFailed,

    // Answer errors
    AnswerGenerationFailed,
    EmptyAnswerResponse,

    // Review errors
    ReviewFailed,
    NoAnswerToReview,
    MaxIterationsReached,

    // External errors
    CopilotError(CopilotError),
    IoError(std::io::Error),

    // Fallback for truly custom errors
    Other(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Intent errors
            AgentError::IntentExtractionFailed => {
                write!(f, "Failed to extract intent from question")
            }
            AgentError::MalformedIntentResponse => {
                write!(f, "Intent extraction produced malformed response")
            }
            AgentError::EmptyIntentResponse => {
                write!(f, "Intent extraction produced empty response")
            }

            // Planning errors
            AgentError::PlanningFailed => write!(f, "Failed to create execution plan"),
            AgentError::EmptyPlan => write!(f, "Generated plan contains no commands"),
            AgentError::InvalidPlanFormat => write!(f, "Generated plan has invalid format"),

            // Command errors
            AgentError::UnknownCommand(cmd) => write!(f, "Unknown command: {cmd}"),
            AgentError::PathNotFound(path) => write!(f, "Path does not exist: {}", path.display()),
            AgentError::PathIsDirectory(path) => {
                write!(f, "Path is a directory: {}", path.display())
            }
            AgentError::CommandExecutionFailed => write!(f, "Command execution failed"),

            // Answer errors
            AgentError::AnswerGenerationFailed => write!(f, "Failed to generate answer"),
            AgentError::EmptyAnswerResponse => write!(f, "Generated answer is empty"),

            // Review errors
            AgentError::ReviewFailed => write!(f, "Failed to review answer"),
            AgentError::NoAnswerToReview => write!(f, "No answer available to review"),
            AgentError::MaxIterationsReached => {
                write!(f, "Maximum iterations reached without satisfactory answer")
            }

            // External errors
            AgentError::CopilotError(err) => write!(f, "Copilot error: {err}"),
            AgentError::IoError(err) => write!(f, "I/O error: {err}"),

            // Fallback
            AgentError::Other(msg) => write!(f, "Other error: {msg}"),
        }
    }
}

impl Error for AgentError {}

impl From<CopilotError> for AgentError {
    fn from(error: CopilotError) -> Self {
        AgentError::CopilotError(error)
    }
}

impl From<std::io::Error> for AgentError {
    fn from(error: std::io::Error) -> Self {
        AgentError::IoError(error)
    }
}

/// Represents the context for an agent session
#[derive(Default)]
struct AgentContext {
    /// The original user question
    question: String,
    /// Commands to execute
    plan: Vec<String>,
    /// Results from executed commands
    command_results: Vec<(String, String)>,
    /// The current generated answer
    current_answer: Option<String>,
    /// The review result
    review_result: Option<String>,
    /// Number of iterations
    iterations: usize,
}

/// Agent that processes user queries to provide answers based on file system commands
pub struct Agent {
    /// Client for GitHub Copilot API access
    client: CopilotClient,
    /// Model ID to use for AI operations
    model_id: String,
    /// Context for the current session
    context: AgentContext,
}

impl Agent {
    /// Creates a new Agent instance with default configuration
    ///
    /// # Returns
    ///
    /// A new Agent instance or an error if initialization fails
    ///
    /// # Errors
    ///
    /// Returns `AgentError::CopilotError` if the Copilot client fails to initialize
    pub async fn new() -> Result<Self, AgentError> {
        // Initialize with default editor version
        let client = CopilotClient::from_env_with_models("1.0.0".to_string())
            .await
            .map_err(AgentError::CopilotError)?;

        // Default model
        let model_id = "gpt-4".to_string();

        Ok(Self {
            client,
            model_id,
            context: AgentContext::default(),
        })
    }

    /// Creates a new Agent with a specified model ID
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model ID to use for Copilot API calls
    ///
    /// # Returns
    ///
    /// A new Agent instance or an error if initialization fails
    ///
    /// # Errors
    ///
    /// Returns `AgentError::CopilotError` if the Copilot client fails to initialize
    pub async fn with_model(model_id: String) -> Result<Self, AgentError> {
        let client = CopilotClient::from_env_with_models("1.0.0".to_string())
            .await
            .map_err(AgentError::CopilotError)?;

        Ok(Self {
            client,
            model_id,
            context: AgentContext::default(),
        })
    }

    /// Process a user query and return an answer
    ///
    /// This method orchestrates the entire agent workflow:
    /// 1. Understanding the question
    /// 2. Planning the execution
    /// 3. Executing commands
    /// 4. Generating an answer
    /// 5. Reviewing the answer
    /// 6. Iterating if necessary
    ///
    /// # Arguments
    ///
    /// * `query` - The user's query string
    ///
    /// # Returns
    ///
    /// The agent's final answer or an error
    ///
    /// # Errors
    ///
    /// Returns various `AgentError` types depending on which step fails
    pub async fn process_query(&mut self, query: &str) -> Result<String, AgentError> {
        // Reset context for new query
        self.context = AgentContext::default();
        self.context.question = query.to_string();

        // Maximum number of iterations to prevent infinite loops

        while self.context.iterations < MAX_ITERATIONS {
            self.context.iterations += 1;

            self.understand_question().await?;
            self.plan_execution().await?;
            self.execute_commands()?;
            self.create_answer().await?;

            let review_passed = self.review_answer().await?;
            if review_passed {
                return Ok(self.context.current_answer.clone().unwrap_or_default());
            }

            println!(
                "Review failed, starting iteration {}",
                self.context.iterations + 1
            );
        }

        // If we've reached the maximum iterations, return the last answer with a note
        if let Some(answer) = &self.context.current_answer {
            Ok(format!(
                "{answer}\n\n(Note: This answer was provided after reaching the maximum number of iteration attempts.)",
            ))
        } else {
            Err(AgentError::Other(
                "Failed to generate an answer after maximum iterations".to_string(),
            ))
        }
    }

    /// Extract intent from user's question
    async fn understand_question(&mut self) -> Result<(), AgentError> {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are an assistant that understands user questions about code repositories. Extract the user's intent regarding what files or directories they want to explore.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Based on this question: '{}', identify what directories and files the user wants to explore. Respond in this format:\n\n{{\"tree\": [\"path1\", \"path2\"], \"show_file\": [\"file1\", \"file2\"]}}",
                    self.context.question
                ),
            },
        ];

        let response = self
            .client
            .chat_completion(messages, self.model_id.clone())
            .await?;

        if let Some(choice) = response.choices.first() {
            println!("Intent extraction: {}", choice.message.content);
            // Here you would parse the JSON response, but for simplicity we'll skip that part
            Ok(())
        } else {
            Err(AgentError::IntentExtractionFailed)
        }
    }

    /// Plan what commands to execute based on extracted intent
    async fn plan_execution(&mut self) -> Result<(), AgentError> {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are an assistant that plans how to answer questions about code repositories. You can use 'tree' to show directory structure and 'show_file' to display file contents.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Based on this question: '{}', create a plan of what commands to run. Return a JSON array of commands like [\"tree src\", \"show_file src/main.rs\"]",
                    self.context.question
                ),
            },
        ];

        let response = self
            .client
            .chat_completion(messages, self.model_id.clone())
            .await?;

        if let Some(choice) = response.choices.first() {
            println!("Plan: {}", choice.message.content);

            // Mock command parsing - in a real implementation, parse JSON from response
            self.context.plan = vec!["tree src".to_string(), "show_file src/main.rs".to_string()];

            if self.context.plan.is_empty() {
                return Err(AgentError::EmptyPlan);
            }

            Ok(())
        } else {
            Err(AgentError::PlanningFailed)
        }
    }

    /// Execute the planned commands
    fn execute_commands(&mut self) -> Result<(), AgentError> {
        self.context.command_results.clear();

        // Execute each command in the plan
        for command in &self.context.plan {
            let cmd_result = if command.starts_with("tree ") {
                let path = command.strip_prefix("tree ").unwrap_or(".");
                let path = Path::new(path);
                
                // Check if path exists
                if !path.exists() {
                    return Err(AgentError::PathNotFound(path.to_path_buf()));
                }
                
                // Directly call the generate_tree function from tree module
                generate_tree(path, "", None, None)
                
            } else if command.starts_with("show_file ") {
                let path = command.strip_prefix("show_file ").unwrap_or("");
                let path = Path::new(path);
                
                // Directly call the read_file_content function from show_file module
                match read_file_content(path) {
                    Ok(content) => content,
                    Err(e) => match e {
                        FileReadError::NotFound => {
                            return Err(AgentError::PathNotFound(path.to_path_buf()));
                        }
                        FileReadError::IsDirectory => {
                            return Err(AgentError::PathIsDirectory(path.to_path_buf()));
                        }
                        FileReadError::Io(io_err) => return Err(AgentError::IoError(io_err)),
                    },
                }
            } else {
                return Err(AgentError::UnknownCommand(command.clone()));
            };

            // Truncate output for logging
            let preview_len = std::cmp::min(100, cmd_result.len());
            println!(
                "Command result ({}): {}{}",
                command,
                &cmd_result[..preview_len],
                if cmd_result.len() > preview_len {
                    "..."
                } else {
                    ""
                }
            );

            self.context
                .command_results
                .push((command.clone(), cmd_result));
        }

        Ok(())
    }

    /// Generate an answer based on command results
    async fn create_answer(&mut self) -> Result<(), AgentError> {
        // Prepare command results for the prompt
        let mut command_results_text = String::new();
        for (cmd, result) in &self.context.command_results {
            command_results_text.push_str(&format!("## Command: {cmd}\n\n```\n{result}\n```\n\n",));
        }

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are an assistant that analyzes code repositories. Create a helpful response based on executed commands.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Question: {}\n\nCommand results:\n\n{}\n\nBased on the above information, please provide a comprehensive answer to the question.",
                    self.context.question,
                    command_results_text
                ),
            },
        ];

        let response = self
            .client
            .chat_completion(messages, self.model_id.clone())
            .await?;
        if let Some(choice) = response.choices.first() {
            self.context.current_answer = Some(choice.message.content.clone());
            println!("Generated answer: {}", choice.message.content);
            Ok(())
        } else {
            Err(AgentError::AnswerGenerationFailed)
        }
    }

    /// Review the generated answer
    async fn review_answer(&mut self) -> Result<bool, AgentError> {
        let Some(answer) = &self.context.current_answer else {
            return Err(AgentError::NoAnswerToReview);
        };

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are a critical reviewer. Evaluate if the answer adequately addresses the question.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Question: {}\n\nAnswer: {}\n\nDoes this answer adequately address the question? Only respond with 'YES' if the answer is adequate, or 'NO: <reason>' if not.",
                    self.context.question,
                    answer
                ),
            },
        ];

        let response = self
            .client
            .chat_completion(messages, self.model_id.clone())
            .await?;
        if let Some(choice) = response.choices.first() {
            let review = choice.message.content.clone();
            self.context.review_result = Some(review.clone());
            println!("Review result: {review}");

            // Simple check if the review is positive
            let passed = review.to_uppercase().starts_with("YES");

            Ok(passed)
        } else {
            Err(AgentError::ReviewFailed)
        }
    }
}