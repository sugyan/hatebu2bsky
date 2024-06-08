mod bsky;
mod client;
mod hatebu;

use encoding_rs::Encoding;
use mime::Mime;
use scraper::{Html, Selector};
use webpage::HTML;
use worker::Env;
use worker::{console_error, console_log, event, Fetch};
use worker::{ScheduleContext, ScheduledEvent};

const KV_NAMESPACE: &str = "kv";

#[event(scheduled)]
async fn main(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    match run(env).await {
        Ok(len) => console_log!("done {len} entries"),
        Err(err) => console_error!("error: {err:?}"),
    }
}

async fn run(env: Env) -> Result<usize, Box<dyn std::error::Error>> {
    let hatena_id = env.var("HATENA_ID").map(|v| v.to_string())?;
    let identifier = env.var("BSKY_IDENTIFIER").map(|v| v.to_string())?;
    let password = env.var("BSKY_PASSWORD").map(|v| v.to_string())?;

    let kv = env.kv(KV_NAMESPACE).expect("failed to get kv");
    // collect new entries from hatena bookmark
    let mut entries = Vec::new();
    for entry in hatebu::list_bookmarks(&hatena_id).await?.iter().rev() {
        console_log!("{} {}", entry.url, entry.title);
        if let Some(text) = kv.get(&entry.url).text().await? {
            console_log!(" -> already exists: {text}");
            continue;
        }
        entries.push(entry.clone());
    }
    if entries.is_empty() {
        return Ok(0);
    }
    // post new entries to bsky
    let agent = bsky::BskyAgent::new(&identifier, &password).await?;
    for entry in &entries {
        console_log!("entry: {:?}", entry);
        match post2bsky(&agent, entry).await {
            Ok(output) => {
                console_log!("done: {:?}", output);
                kv.put(&entry.url, output)
                    .expect("failed to put")
                    .execute()
                    .await?;
            }
            Err(err) => console_error!("error: {:?}", err),
        }
    }
    Ok(entries.len())
}

async fn post2bsky(
    agent: &bsky::BskyAgent,
    entry: &hatebu::Entry,
) -> Result<atrium_api::com::atproto::repo::create_record::Output, Box<dyn std::error::Error>> {
    let html = get_webpage(&entry.url).await?;
    agent.create_post(entry, &html).await
}

async fn get_webpage(url: &str) -> Result<HTML, Box<dyn std::error::Error>> {
    let mut res = Fetch::Url(url.parse()?).send().await?;
    let content_type = res
        .headers()
        .get(http::header::CONTENT_TYPE.as_str())?
        .and_then(|value| value.parse::<Mime>().ok());
    let bytes = res.bytes().await?;
    let s = if let Some(encoding) = content_type
        .as_ref()
        .and_then(|mime| {
            mime.get_param("charset")
                .map(|charset| charset.as_str().to_string())
        })
        .or_else(|| extract_charset(bytes.as_ref()))
        .and_then(|charset| Encoding::for_label(charset.as_bytes()))
    {
        encoding.decode(bytes.as_ref()).0
    } else {
        String::from_utf8_lossy(bytes.as_ref())
    };
    Ok(HTML::from_string(s.to_string(), Some(url.into()))?)
}

fn extract_charset(bytes: &[u8]) -> Option<String> {
    let html = Html::parse_document(&String::from_utf8_lossy(bytes));
    let selector = Selector::parse("meta[charset], meta[http-equiv=content-type]").unwrap();
    for element in html.select(&selector) {
        if let Some(charset) = element.value().attr("charset") {
            return Some(charset.to_string());
        }
        if let Some(content) = element.value().attr("content") {
            let content = content.to_lowercase();
            if let Some(index) = content.find("charset=") {
                let charset = content[index + 8..].trim();
                return Some(charset.to_string());
            }
        }
    }
    None
}
