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
    let num_width = articles.len().to_string().len();

    for (i, a) in articles.iter().enumerate() {
        let id_short = &a.article_id[..8.min(a.article_id.len())];
        let feed_name = feed_names
            .get(&a.feed_id)
            .map(|s| s.as_str())
            .unwrap_or("?");
        let title = a.title.as_deref().unwrap_or("(no title)");
        let date_str = format_timestamp(a.date_published.unwrap_or(a.date_arrived));
        let num = format!("{:>width$}.", i + 1, width = num_width);
        let star = if a.starred { " *" } else { "" };

        let url = a.best_url().unwrap_or("");
        let indent = num_width + 2;

        if a.read {
            let _ = writeln!(out, "{} {}", num.dimmed(), title.dimmed());
            let _ = writeln!(out, "{:indent$}{}", "", url.dimmed(), indent = indent);
            let _ = writeln!(
                out,
                "{:indent$}{} · {} · {}{}",
                "",
                feed_name.dimmed(),
                date_str.dimmed(),
                id_short.dimmed(),
                star.dimmed(),
                indent = indent,
            );
        } else {
            let _ = writeln!(out, "{} {}", num.bold(), title.bold());
            let _ = writeln!(
                out,
                "{:indent$}{}",
                "",
                url.blue().underline(),
                indent = indent,
            );
            let _ = writeln!(
                out,
                "{:indent$}{} · {} · {}{}",
                "",
                feed_name.green(),
                date_str,
                id_short.dimmed(),
                star.yellow(),
                indent = indent,
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
    if let Some(url) = article.best_url() {
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

    let body = article
        .content_text
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            article
                .content_html
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|html| html2text::from_read(html.as_bytes(), 80))
        })
        .or_else(|| {
            article
                .summary
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|html| html2text::from_read(html.as_bytes(), 80))
        });

    match body {
        Some(text) if !text.trim().is_empty() => println!("{}", text.trim()),
        _ => println!("(no content)"),
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
