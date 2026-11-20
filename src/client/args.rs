use clap::{Args, Subcommand};

#[derive(Args, Debug, Clone, Default)]
pub struct Out {
    #[arg(long, value_name = "FIELDS", num_args = 0..=1, default_missing_value = "", help = "Print JSON (optionally only these comma-separated fields)")]
    pub json: Option<String>,
    #[arg(
        long,
        value_name = "FILTER",
        help = "Print parts of the JSON answer, for example .cards[].title"
    )]
    pub jq: Option<String>,
    #[arg(
        long,
        help = "Server address (default: from tazko auth login or TAZKO_HOST)"
    )]
    pub host: Option<String>,
    #[arg(
        long,
        help = "Company slug (default: your only company or tazko config set company)"
    )]
    pub company: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum ClientCommand {
    #[command(about = "Sign in with your browser, see who you are, print a token for scripts")]
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    #[command(about = "Projects you belong to and who is in them")]
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    #[command(about = "Boards in your projects")]
    Board {
        #[command(subcommand)]
        action: BoardAction,
    },
    #[command(about = "Cards: list, view, create, move, mark done, comment, archive")]
    Card {
        #[command(subcommand)]
        action: CardAction,
    },
    #[command(about = "Cards and checklist items assigned to you")]
    My {
        #[command(subcommand)]
        action: MyAction,
    },
    #[command(about = "Search cards with the same operators as the app")]
    Search {
        query: String,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Call any API endpoint, for example tazko api GET /me")]
    Api {
        method: String,
        path: String,
        #[arg(long, help = "JSON body to send")]
        data: Option<String>,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Default server and company")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    #[command(about = "Sign in through the browser with a one-time code")]
    Login {
        #[arg(long, help = "Server address, for example https://tazko.example.com")]
        host: Option<String>,
        #[arg(long, help = "Print the address instead of opening the browser")]
        no_browser: bool,
    },
    #[command(about = "Forget the token stored on this computer")]
    Logout,
    #[command(about = "Show who is signed in, on which server")]
    Status {
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Print the stored token, for scripts")]
    Token,
}

#[derive(Subcommand, Debug)]
pub enum ProjectAction {
    #[command(about = "Projects you belong to")]
    List {
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "People in a project")]
    Members {
        project: i64,
        #[command(flatten)]
        out: Out,
    },
}

#[derive(Subcommand, Debug)]
pub enum BoardAction {
    #[command(about = "Boards you can see")]
    List {
        #[arg(long, help = "Only this project (id)")]
        project: Option<i64>,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Lists and cards on a board")]
    View {
        board: String,
        #[arg(long, help = "Open it in the browser")]
        web: bool,
        #[command(flatten)]
        out: Out,
    },
}

#[derive(Subcommand, Debug)]
pub enum CardAction {
    #[command(about = "Cards on a board")]
    List {
        #[arg(long, help = "Board id or name")]
        board: String,
        #[arg(long, help = "Only this list (id or name)")]
        list: Option<String>,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "One card, as BOARD/CARD, for example 3/142")]
    View {
        card: String,
        #[arg(long, help = "Open it in the browser")]
        web: bool,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Add a card")]
    Create {
        #[arg(long, help = "Board id or name")]
        board: String,
        #[arg(long, help = "List id or name (default: the first list)")]
        list: Option<String>,
        #[arg(long)]
        title: String,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Move a card to another list on its board")]
    Move {
        card: String,
        #[arg(long, help = "List id or name")]
        to: String,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Mark a card done (or not done with --undo)")]
    Done {
        card: String,
        #[arg(long)]
        undo: bool,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Comment on a card")]
    Comment {
        card: String,
        #[arg(long)]
        body: String,
        #[command(flatten)]
        out: Out,
    },
    #[command(about = "Archive a card")]
    Archive {
        card: String,
        #[command(flatten)]
        out: Out,
    },
}

#[derive(Subcommand, Debug)]
pub enum MyAction {
    #[command(about = "Cards assigned to you")]
    Cards {
        #[command(flatten)]
        out: Out,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    #[command(about = "Show the settings")]
    Get,
    #[command(about = "Set host or company")]
    Set { key: String, value: String },
}
