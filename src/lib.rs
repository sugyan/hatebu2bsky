mod bsky;
mod client;
mod hatebu;

use worker::{console_error, console_log, event};
use worker::{Env, ScheduleContext, ScheduledEvent};

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
    let response = reqwest::get(&entry.url).await?;
    let html = webpage::HTML::from_string(response.text().await?, None)?;
    agent.create_post(entry, &html).await
}
