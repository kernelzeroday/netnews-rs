use std::collections::HashMap;
use std::io::Write;

use chrono::{DateTime, Local, Utc};
use colored::Colorize;

use crate::db::ArticleRow;
use crate::opml::Subscriptions;

pub fn print_feeds(subs: &Subscriptions) {
    for folder in &subs.folders {
        println!("{}", folder.name.bold());
        for feed in &folder.feeds {
            println!("  {} {}", feed.text, feed.xml_url.dimmed());
        }
        println!();
    }
}

pub fn print_articles(articles: &[ArticleRow], feed_names: &HashMap<String, String>) {
    if articles.is_empty() {
        println!("No articles found.");
        return;
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for a in articles {
        let id_short = &a.article_id[..8.min(a.article_id.len())];
        let feed_name = feed_names
            .get(&a.feed_id)
            .map(|s| s.as_str())
            .unwrap_or("?");
        let title = a.title.as_deref().unwrap_or("(no title)");
        let title_display = truncate(title, 60);
        let date_str = format_timestamp(a.date_published.unwrap_or(a.date_arrived));

        let star = if a.starred { "*" } else { " " };

        if a.read {
            let _ = writeln!(
                out,
                "{} {} {:15} {:60} {}",
                star,
                id_short.dimmed(),
                truncate(feed_name, 15).dimmed(),
                title_display.dimmed(),
                date_str.dimmed(),
            );
        } else {
            let _ = writeln!(
                out,
                "{} {} {:15} {:60} {}",
                star.yellow(),
                id_short.cyan(),
                truncate(feed_name, 15).green(),
                title_display.bold(),
                date_str,
            );
        }
    }
}

pub fn print_article_detail(article: &ArticleRow, feed_name: &str) {
    let title = article.title.as_deref().unwrap_or("(no title)");
    let date_str = format_timestamp(article.date_published.unwrap_or(article.date_arrived));

    println!("{}", title.bold());
    println!(
        "{}  {}  {}",
        feed_name.green(),
        date_str,
        &article.article_id[..8.min(article.article_id.len())].dimmed()
    );
    if let Some(url) = &article.url {
        println!("{}", url.blue().underline());
    }

    let authors = parse_author_names(&article.authors_json);
    if !authors.is_empty() {
        println!("By {}", authors.join(", "));
    }

    let mut status = Vec::new();
    if article.starred {
        status.push("starred");
    }
    if !article.read {
        status.push("unread");
    }
    if !status.is_empty() {
        println!("[{}]", status.join(", "));
    }

    println!("{}", "─".repeat(72));

    if let Some(text) = &article.content_text {
        println!("{}", text.trim());
    } else if let Some(html) = &article.content_html {
        let text = html2text::from_read(html.as_bytes(), 80);
        println!("{}", text.trim());
    } else if let Some(summary) = &article.summary {
        let text = html2text::from_read(summary.as_bytes(), 80);
        println!("{}", text.trim());
    } else {
        println!("(no content)");
    }
}

fn parse_author_names(json: &Option<String>) -> Vec<String> {
    let Some(json) = json else { return vec![] };
    let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(json) else {
        return vec![];
    };
    arr.iter()
        .filter_map(|v| v.get("name")?.as_str().map(String::from))
        .collect()
}

fn format_timestamp(ts: f64) -> String {
    let Some(dt) = DateTime::<Utc>::from_timestamp(ts as i64, 0) else {
        return "?".to_string();
    };
    let local: DateTime<Local> = dt.into();
    local.format("%b %d %H:%M").to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{}…", truncated)
    }
}
