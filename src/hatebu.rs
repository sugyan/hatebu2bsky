use rss::{Channel, Item};
use std::error::Error;
use worker::Fetch;

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

pub async fn list_bookmarks(hatena_id: &str) -> Result<Vec<Entry>, Box<dyn Error>> {
    let url = format!("https://b.hatena.ne.jp/{hatena_id}/bookmark.rss");
    let mut entries = Vec::new();
    for item in get_items(&url).await? {
        entries.push(Entry {
            title: item.title.expect("item has no title"),
            url: item.link.expect("item has no link"),
            description: item.description,
            tags: item
                .dublin_core_ext
                .map(|ext| ext.subjects)
                .unwrap_or_default(),
        });
    }
    Ok(entries)
}

async fn get_items(url: &str) -> Result<Vec<Item>, Box<dyn Error>> {
    Ok(Channel::read_from(
        Fetch::Url(url.parse()?)
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
    )?
    .items)
}
