use std::io::Write;
use clap::{Parser,Subcommand};
use std::io;
use tracing::error;

mod agent;

#[derive(Parser)]
#[command(about)]
struct Cli{
    #[command(subcommand)]
    command: CliCommand,
}
#[derive(Subcommand)]
enum CliCommand{
    /// One shot query to the agent
    Ask{
        /// Search query
        #[arg(short, long)]
        query: String,
    },
    /// Chat with the agent
    Chat{},
}

#[tokio::main]
async fn main() {
    // tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        CliCommand::Ask { query } => ask_question(&query).await,
        CliCommand::Chat {} => chat_with_agent().await,
    };
}

async fn ask_question(query: &str) {
    let mut agent = match agent::Agent::build().await{
        Ok(agent) => agent,
        Err(err) => {
            error!("Could not build agent: {}", err);
            return;
        }
    };
    let answer = match agent.ask_one(query).await {
        Ok(answer) => answer,
        Err(err) => {
            error!("Error in agent.ask_question: {}", err);
            return;
        }
    };
    println!("{answer}");
}

async fn chat_with_agent() {
    let mut agent = agent::Agent::build().await
        .expect("could not build agent");

    // interactive loop
    loop {
        let mut buf = String::new();
        // show a cursor
        print!("\n:> ");
        io::stdout().flush().unwrap();
        
        // read input
        io::stdin().read_line(&mut buf).unwrap();
        match buf.trim() {
            "exit" | "quit" => break,
            query => {
                if !query.is_empty() {
                    let answer = agent.ask_one(query).await.expect("Error in ask_one");
                    println!("{answer}");
                }
            }
        }
    }
}
