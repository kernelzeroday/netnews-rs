use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};

pub struct ArticleRow {
    pub article_id: String,
    pub feed_id: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub date_published: Option<f64>,
    pub authors_json: Option<String>,
    pub read: bool,
    pub starred: bool,
    pub date_arrived: f64,
}

pub struct ArticleFilter {
    pub feed_ids: Option<Vec<String>>,
    pub unread: bool,
    pub starred: bool,
    pub limit: usize,
}

pub struct NewArticle {
    pub article_id: String,
    pub feed_id: String,
    pub unique_id: String,
    pub title: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub url: Option<String>,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub date_published: Option<f64>,
    pub authors_json: Option<String>,
}

pub struct NnwDb {
    conn: Connection,
}

impl NnwDb {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self { conn })
    }

    pub fn open_readonly(db_path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self { conn })
    }

    pub fn articles(&self, filter: &ArticleFilter) -> Result<Vec<ArticleRow>> {
        let mut sql = String::from(
            "SELECT a.articleID, a.feedID, a.title, a.url, a.contentHTML, a.contentText, \
             a.summary, a.imageURL, a.datePublished, a.authors, \
             s.read, s.starred, s.dateArrived \
             FROM articles a \
             JOIN statuses s ON a.articleID = s.articleID \
             WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(feed_ids) = &filter.feed_ids {
            if feed_ids.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders: Vec<&str> = feed_ids.iter().map(|_| "?").collect();
            sql.push_str(&format!(" AND a.feedID IN ({})", placeholders.join(",")));
            for id in feed_ids {
                param_values.push(Box::new(id.clone()));
            }
        }
        if filter.unread {
            sql.push_str(" AND s.read = 0");
        }
        if filter.starred {
            sql.push_str(" AND s.starred = 1");
        }
        sql.push_str(" ORDER BY COALESCE(a.datePublished, s.dateArrived) DESC LIMIT ?");
        param_values.push(Box::new(filter.limit as i64));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_ref.as_slice(), row_to_article)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn article_by_prefix(&self, prefix: &str) -> Result<ArticleRow> {
        let sql = "SELECT a.articleID, a.feedID, a.title, a.url, a.contentHTML, a.contentText, \
                   a.summary, a.imageURL, a.datePublished, a.authors, \
                   s.read, s.starred, s.dateArrived \
                   FROM articles a \
                   JOIN statuses s ON a.articleID = s.articleID \
                   WHERE a.articleID LIKE ?1";
        let pattern = format!("{}%", prefix);
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows: Vec<ArticleRow> = stmt
            .query_map(params![pattern], row_to_article)?
            .collect::<Result<Vec<_>, _>>()?;

        match rows.len() {
            0 => anyhow::bail!("No article found matching '{}'", prefix),
            1 => Ok(rows.remove(0)),
            n => anyhow::bail!(
                "Ambiguous prefix '{}' matches {} articles. Use a longer prefix.",
                prefix,
                n
            ),
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ArticleRow>> {
        let sql = "SELECT a.articleID, a.feedID, a.title, a.url, a.contentHTML, a.contentText, \
                   a.summary, a.imageURL, a.datePublished, a.authors, \
                   s.read, s.starred, s.dateArrived \
                   FROM articles a \
                   JOIN statuses s ON a.articleID = s.articleID \
                   JOIN search ON a.searchRowID = search.rowid \
                   WHERE search MATCH ?1 \
                   ORDER BY COALESCE(a.datePublished, s.dateArrived) DESC \
                   LIMIT ?2";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![query, limit as i64], row_to_article)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn set_read(&self, article_id: &str, read: bool) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE statuses SET read = ?1 WHERE articleID = ?2",
            params![read, article_id],
        )?;
        if changed == 0 {
            anyhow::bail!("No status row for article '{}'", article_id);
        }
        Ok(())
    }

    pub fn set_starred(&self, article_id: &str, starred: bool) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE statuses SET starred = ?1 WHERE articleID = ?2",
            params![starred, article_id],
        )?;
        if changed == 0 {
            anyhow::bail!("No status row for article '{}'", article_id);
        }
        Ok(())
    }

    pub fn insert_articles(&self, articles: &[NewArticle]) -> Result<usize> {
        let mut inserted = 0usize;
        let tx = self.conn.unchecked_transaction()?;

        for a in articles {
            let already_exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM articles WHERE articleID = ?1)",
                params![a.article_id],
                |row| row.get(0),
            )?;
            if already_exists {
                continue;
            }

            // Insert into FTS search index
            tx.execute(
                "INSERT INTO search (title, body) VALUES (?1, ?2)",
                params![
                    a.title.as_deref().unwrap_or(""),
                    a.content_text.as_deref().unwrap_or("")
                ],
            )?;
            let search_row_id = tx.last_insert_rowid();

            tx.execute(
                "INSERT INTO articles (articleID, feedID, uniqueID, title, contentHTML, contentText, \
                 url, summary, imageURL, datePublished, authors, searchRowID) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    a.article_id,
                    a.feed_id,
                    a.unique_id,
                    a.title,
                    a.content_html,
                    a.content_text,
                    a.url,
                    a.summary,
                    a.image_url,
                    a.date_published,
                    a.authors_json,
                    search_row_id,
                ],
            )?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            tx.execute(
                "INSERT OR IGNORE INTO statuses (articleID, read, starred, dateArrived) VALUES (?1, 0, 0, ?2)",
                params![a.article_id, now],
            )?;

            inserted += 1;
        }

        tx.commit()?;
        Ok(inserted)
    }
}

fn row_to_article(row: &rusqlite::Row) -> rusqlite::Result<ArticleRow> {
    Ok(ArticleRow {
        article_id: row.get(0)?,
        feed_id: row.get(1)?,
        title: row.get(2)?,
        url: row.get(3)?,
        content_html: row.get(4)?,
        content_text: row.get(5)?,
        summary: row.get(6)?,
        image_url: row.get(7)?,
        date_published: row.get(8)?,
        authors_json: row.get(9)?,
        read: row.get(10)?,
        starred: row.get(11)?,
        date_arrived: row.get(12)?,
    })
}
