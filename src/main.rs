use clap::{Parser, Subcommand};
use nishiogi::agent::Agent;
use std::process;
use tokio;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ask a question about the codebase
    Ask {
        /// The question you want to ask
        #[arg(required = true)]
        question: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Ask { question } => {
            println!("Processing question: {}", question);
            
            // Initialize the agent
            let mut agent = match Agent::new().await {
                Ok(agent) => agent,
                Err(err) => {
                    eprintln!("Failed to initialize agent: {}", err);
                    process::exit(1);
                }
            };
            
            // Process the question
            match agent.process_query(question).await {
                Ok(answer) => {
                    println!("\n=== Answer ===\n");
                    println!("{}", answer);
                }
                Err(err) => {
                    eprintln!("Error processing query: {}", err);
                    process::exit(1);
                }
            }
        }
    }
}