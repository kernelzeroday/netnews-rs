mod config;
mod db;
mod display;
mod feed;
mod opml;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use db::{ArticleFilter, NnwDb};
use opml::Subscriptions;

#[derive(Parser)]
#[command(name = "netnews", about = "Command-line interface for NetNewsWire")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Account name
    #[arg(long, default_value = "OnMyMac", global = true)]
    account: String,
}

#[derive(Subcommand)]
enum Command {
    /// List feeds organized by folder
    Feeds,

    /// List articles
    #[command(alias = "ls")]
    Articles {
        /// Filter by feed name
        #[arg(long)]
        feed: Option<String>,
        /// Filter by folder name
        #[arg(long)]
        folder: Option<String>,
        /// Show only unread articles
        #[arg(long)]
        unread: bool,
        /// Show only starred articles
        #[arg(long)]
        starred: bool,
        /// Maximum number of articles
        #[arg(short, long, default_value = "50")]
        limit: usize,
    },

    /// Read an article (prefix match on ID)
    Read {
        /// Article ID or prefix
        id: String,
    },

    /// Full-text search across articles
    Search {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Mark article read/unread/starred/unstarred
    Mark {
        #[command(subcommand)]
        action: MarkAction,
    },

    /// Add a new feed subscription
    Add {
        /// Feed URL (RSS/Atom)
        url: String,
        /// Folder to add the feed to
        #[arg(long)]
        folder: Option<String>,
        /// Display name for the feed
        #[arg(long)]
        name: Option<String>,
    },

    /// Remove a feed subscription
    Remove {
        /// Feed name or URL
        feed: String,
    },

    /// Fetch new articles from feeds
    Refresh {
        /// Specific feed name (refreshes all if omitted)
        feed: Option<String>,
    },
}

#[derive(Subcommand)]
enum MarkAction {
    /// Mark article as read
    Read {
        /// Article ID or prefix
        id: String,
    },
    /// Mark article as unread
    Unread {
        /// Article ID or prefix
        id: String,
    },
    /// Star an article
    Star {
        /// Article ID or prefix
        id: String,
    },
    /// Unstar an article
    Unstar {
        /// Article ID or prefix
        id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let account = &cli.account;

    match cli.command {
        None => cmd_articles(account, None, None, false, false, 50),
        Some(Command::Feeds) => cmd_feeds(account),
        Some(Command::Articles {
            feed,
            folder,
            unread,
            starred,
            limit,
        }) => cmd_articles(account, feed, folder, unread, starred, limit),
        Some(Command::Read { id }) => cmd_read(account, &id),
        Some(Command::Search { query, limit }) => cmd_search(account, &query, limit),
        Some(Command::Mark { action }) => cmd_mark(account, action),
        Some(Command::Add { url, folder, name }) => cmd_add(account, &url, folder.as_deref(), name),
        Some(Command::Remove { feed }) => cmd_remove(account, &feed),
        Some(Command::Refresh { feed }) => cmd_refresh(account, feed.as_deref()),
    }
}

fn cmd_feeds(account: &str) -> Result<()> {
    let subs = Subscriptions::load(&config::opml_path(account)?)?;
    display::print_feeds(&subs);
    Ok(())
}

fn cmd_articles(
    account: &str,
    feed_name: Option<String>,
    folder_name: Option<String>,
    unread: bool,
    starred: bool,
    limit: usize,
) -> Result<()> {
    let subs = Subscriptions::load(&config::opml_path(account)?)?;
    let db = NnwDb::open_readonly(&config::db_path(account)?)?;

    let feed_ids = match (&feed_name, &folder_name) {
        (Some(name), _) => {
            let url = subs
                .feed_url_for_name(name)
                .with_context(|| format!("Feed '{}' not found", name))?;
            Some(vec![url])
        }
        (_, Some(folder)) => {
            let urls = subs.feed_urls_in_folder(folder);
            if urls.is_empty() {
                anyhow::bail!("Folder '{}' not found or empty", folder);
            }
            Some(urls)
        }
        _ => None,
    };

    let filter = ArticleFilter {
        feed_ids,
        unread,
        starred,
        limit,
    };
    let articles = db.articles(&filter)?;
    let feed_names = subs.feed_name_map();
    display::print_articles(&articles, &feed_names);
    Ok(())
}

fn cmd_read(account: &str, id: &str) -> Result<()> {
    let subs = Subscriptions::load(&config::opml_path(account)?)?;
    let db = NnwDb::open_readonly(&config::db_path(account)?)?;
    let article = db.article_by_prefix(id)?;
    let feed_names = subs.feed_name_map();
    let feed_name = feed_names
        .get(&article.feed_id)
        .map(|s| s.as_str())
        .unwrap_or("Unknown");
    display::print_article_detail(&article, feed_name);
    Ok(())
}

fn cmd_search(account: &str, query: &str, limit: usize) -> Result<()> {
    let subs = Subscriptions::load(&config::opml_path(account)?)?;
    let db = NnwDb::open_readonly(&config::db_path(account)?)?;
    let articles = db.search(query, limit)?;
    let feed_names = subs.feed_name_map();
    display::print_articles(&articles, &feed_names);
    Ok(())
}

fn cmd_mark(account: &str, action: MarkAction) -> Result<()> {
    let db = NnwDb::open(&config::db_path(account)?)?;
    match action {
        MarkAction::Read { id } => {
            let article = db.article_by_prefix(&id)?;
            db.set_read(&article.article_id, true)?;
            println!("Marked {} as read", &article.article_id[..8]);
        }
        MarkAction::Unread { id } => {
            let article = db.article_by_prefix(&id)?;
            db.set_read(&article.article_id, false)?;
            println!("Marked {} as unread", &article.article_id[..8]);
        }
        MarkAction::Star { id } => {
            let article = db.article_by_prefix(&id)?;
            db.set_starred(&article.article_id, true)?;
            println!("Starred {}", &article.article_id[..8]);
        }
        MarkAction::Unstar { id } => {
            let article = db.article_by_prefix(&id)?;
            db.set_starred(&article.article_id, false)?;
            println!("Unstarred {}", &article.article_id[..8]);
        }
    }
    Ok(())
}

fn cmd_add(account: &str, url: &str, folder: Option<&str>, name: Option<String>) -> Result<()> {
    let opml_path = config::opml_path(account)?;
    let mut subs = Subscriptions::load(&opml_path)?;

    if subs.all_feeds().iter().any(|(_, f)| f.xml_url == url) {
        anyhow::bail!("Feed '{}' is already subscribed", url);
    }

    let display_name = match name {
        Some(n) => n,
        None => {
            eprint!("Fetching feed to discover name... ");
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("NetNewsWire-CLI/0.1")
                .build()?;
            match client.get(url).send().and_then(|r| r.bytes()) {
                Ok(bytes) => match feed_rs::parser::parse(&bytes[..]) {
                    Ok(parsed) => {
                        eprintln!("done.");
                        parsed
                            .title
                            .map(|t| t.content)
                            .unwrap_or_else(|| url.to_string())
                    }
                    Err(_) => {
                        eprintln!("couldn't parse, using URL as name.");
                        url.to_string()
                    }
                },
                Err(_) => {
                    eprintln!("couldn't fetch, using URL as name.");
                    url.to_string()
                }
            }
        }
    };

    subs.add_feed(
        opml::Feed {
            text: display_name.clone(),
            xml_url: url.to_string(),
            html_url: String::new(),
        },
        folder,
    );
    subs.save(&opml_path)?;

    println!(
        "Added '{}' to {}",
        display_name,
        folder.unwrap_or("Uncategorized")
    );
    Ok(())
}

fn cmd_remove(account: &str, feed: &str) -> Result<()> {
    let opml_path = config::opml_path(account)?;
    let mut subs = Subscriptions::load(&opml_path)?;

    if subs.remove_feed(feed) {
        subs.save(&opml_path)?;
        println!("Removed '{}'", feed);
    } else {
        anyhow::bail!("Feed '{}' not found", feed);
    }
    Ok(())
}

fn cmd_refresh(account: &str, feed_name: Option<&str>) -> Result<()> {
    let subs = Subscriptions::load(&config::opml_path(account)?)?;
    let db = NnwDb::open(&config::db_path(account)?)?;

    let all_feeds = subs.all_feeds();
    let feeds_to_refresh: Vec<(&str, &str)> = match feed_name {
        Some(name) => {
            let name_lower = name.to_lowercase();
            let found = all_feeds
                .iter()
                .find(|(_, f)| f.text.to_lowercase() == name_lower)
                .with_context(|| format!("Feed '{}' not found", name))?;
            vec![(found.1.text.as_str(), found.1.xml_url.as_str())]
        }
        None => all_feeds
            .iter()
            .map(|(_, f)| (f.text.as_str(), f.xml_url.as_str()))
            .collect(),
    };

    let mut total_new = 0usize;

    for (name, url) in &feeds_to_refresh {
        eprint!("  {} ...", name);
        match feed::fetch_feed(url) {
            Ok(articles) => {
                let count = db.insert_articles(&articles)?;
                total_new += count;
                if count > 0 {
                    eprintln!(" {} new", count);
                } else {
                    eprintln!(" up to date");
                }
            }
            Err(e) => {
                eprintln!(" error: {:#}", e);
            }
        }
    }

    println!("{} new articles total", total_new);
    Ok(())
}
