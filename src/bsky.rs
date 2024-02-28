use crate::hatebu::Entry;
use atrium_api::agent::{store::MemorySessionStore, AtpAgent};
use atrium_api::app::bsky::embed::external;
use atrium_api::app::bsky::feed::post::{Record, RecordEmbedEnum};
use atrium_api::com::atproto::repo::create_record::{Input, Output};
use atrium_api::types::string::{Datetime, Did};
use atrium_xrpc_client::reqwest::ReqwestClient;
use webpage::HTML;

pub(crate) struct BskyAgent {
    agent: AtpAgent<MemorySessionStore, ReqwestClient>,
    did: Did,
}

impl BskyAgent {
    pub async fn new(
        identifier: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let agent = AtpAgent::new(
            ReqwestClient::new("https://bsky.social"),
            MemorySessionStore::default(),
        );
        let session = agent.login(identifier, password).await?;
        let did = session.did;
        Ok(Self { agent, did })
    }
    pub async fn create_post(
        &self,
        entry: &Entry,
        html: &HTML,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        let record = Record {
            created_at: Datetime::now(),
            embed: Some(self.embed(entry, html).await?),
            entities: None,
            facets: None,
            labels: None,
            langs: Some(vec!["ja".parse().expect("invalid language")]),
            reply: None,
            tags: None,
            text: text(entry),
        };
        Ok(self
            .agent
            .api
            .com
            .atproto
            .repo
            .create_record(Input {
                collection: "app.bsky.feed.post".parse().expect("invalid collection"),
                record: atrium_api::records::Record::AppBskyFeedPost(Box::new(record)),
                repo: self.did.clone().into(),
                rkey: None,
                swap_commit: None,
                validate: None,
            })
            .await?)
    }

    async fn embed(
        &self,
        entry: &Entry,
        html: &HTML,
    ) -> Result<RecordEmbedEnum, Box<dyn std::error::Error>> {
        let thumb = if let Some(object) = html.opengraph.images.first() {
            let data = reqwest::get(&object.url).await?.bytes().await?.to_vec();
            let uploaded = self.agent.api.com.atproto.repo.upload_blob(data).await?;
            Some(uploaded.blob)
        } else {
            None
        };
        Ok(RecordEmbedEnum::AppBskyEmbedExternalMain(Box::new(
            external::Main {
                external: external::External {
                    description: html
                        .opengraph
                        .properties
                        .get("description")
                        .cloned()
                        .or(html.description.clone())
                        .unwrap_or_default(),
                    thumb,
                    title: html
                        .opengraph
                        .properties
                        .get("title")
                        .cloned()
                        .or(html.title.clone())
                        .unwrap_or_default(),
                    uri: entry.url.clone(),
                },
            },
        )))
    }
}

fn text(entry: &Entry) -> String {
    let mut ret = if let Some(description) = &entry.description {
        format!("{description} / {}", entry.title)
    } else {
        entry.title.clone()
    };
    if !entry.tags.is_empty() {
        ret += "\n";
        ret += &entry
            .tags
            .iter()
            .map(|tag| format!("#{tag}"))
            .collect::<Vec<_>>()
            .join(" ");
    }
    ret
}