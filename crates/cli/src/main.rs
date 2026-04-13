use clap::{Parser, Subcommand};

mod commands;
mod output;

#[derive(Parser)]
#[command(name = "codeindex", about = "Semantic code indexing for AI agents", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new codeindex in the current directory
    Init,

    /// Index the current project
    Index {
        /// Only re-index changed files
        #[arg(long)]
        incremental: bool,

        /// Suppress progress output
        #[arg(long, short)]
        quiet: bool,
    },

    /// Show status of the current index
    Status,

    /// Query the index
    Query {
        /// The query string
        query: String,

        /// Number of results to return
        #[arg(long, default_value = "5")]
        top: usize,

        /// Dependency expansion depth
        #[arg(long, default_value = "1")]
        depth: usize,

        /// Disable query enhancement
        #[arg(long)]
        no_enhance: bool,

        /// Exclude code snippets from results
        #[arg(long)]
        no_code: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Output format: human, compact, json
        #[arg(long, default_value = "human")]
        format: String,
    },

    /// Start the MCP server (stub)
    McpServer,

    /// Manage configuration (stub)
    Config,

    /// Garbage collect the index (stub)
    Gc,

    /// Show index statistics (stub)
    Stats,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => commands::init::run(),
        Commands::Index { incremental, quiet } => commands::index::run(incremental, quiet),
        Commands::Status => commands::status::run(),
        Commands::Query {
            query,
            top,
            depth,
            no_enhance: _,
            no_code,
            json,
            format,
        } => commands::query::run(query, top, depth, !no_code, json, format),
        Commands::McpServer => {
            println!("MCP server not yet implemented.");
            Ok(())
        }
        Commands::Config => {
            println!("Config management not yet implemented.");
            Ok(())
        }
        Commands::Gc => {
            println!("Garbage collection not yet implemented.");
            Ok(())
        }
        Commands::Stats => {
            println!("Stats not yet implemented.");
            Ok(())
        }
    };

    match result {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(2);
        }
    }
}
