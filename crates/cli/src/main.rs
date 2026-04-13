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

    /// Start the MCP server
    McpServer,

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Garbage collect the index
    Gc,

    /// Show index statistics
    Stats,

    /// Watch for file changes and re-index
    Watch {
        /// Stop running daemon
        #[arg(long)]
        stop: bool,
    },

    /// Launch the web dashboard
    Ui {
        /// Port to serve on
        #[arg(long, default_value = "3742")]
        port: u16,
        /// Don't auto-open browser
        #[arg(long)]
        no_open: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Set a config value (e.g. `codeindex config set embedding.provider voyage`)
    Set { key: String, value: String },
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
            eprintln!("Use the standalone mcp-server binary: codeindex-mcp-server");
            Ok(())
        }
        Commands::Config { action } => match action {
            Some(ConfigAction::Set { key, value }) => {
                commands::config_cmd::run_set(&key, &value)
            }
            None => commands::config_cmd::run_show(),
        },
        Commands::Gc => commands::gc::run(),
        Commands::Stats => commands::stats::run(),
        Commands::Watch { stop } => commands::watch::run(stop),
        Commands::Ui { port, no_open } => commands::ui::run(port, no_open),
    };

    match result {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(2);
        }
    }
}
