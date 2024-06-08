use crate::client::{ClientInfo, FetchClient};
use crate::hatebu::Entry;
use atrium_api::app::bsky::embed::external;
use atrium_api::app::bsky::feed::post::RecordEmbedRefs;
use atrium_api::app::bsky::richtext::facet;
use atrium_api::client::AtpServiceClient;
use atrium_api::com::atproto::repo::create_record::{Input, Output};
use atrium_api::did_doc::DidDocument;
use atrium_api::records::{KnownRecord, Record};
use atrium_api::types::string::{Datetime, Did};
use atrium_api::types::Union;
use http::Uri;
use std::sync::{Arc, RwLock};
use webpage::HTML;
use worker::{Fetch, Url};

pub(crate) struct BskyAgent {
    client: AtpServiceClient<FetchClient>,
    did: Did,
}

impl BskyAgent {
    pub async fn new(
        identifier: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let info = Arc::new(RwLock::new(ClientInfo {
            access_jwt: None,
            base_uri: "https://bsky.social".into(),
        }));
        let client = AtpServiceClient::new(FetchClient::new(info.clone()));
        let session = client
            .service
            .com
            .atproto
            .server
            .create_session(atrium_api::com::atproto::server::create_session::Input {
                auth_factor_token: None,
                identifier: identifier.as_ref().to_string(),
                password: password.as_ref().to_string(),
            })
            .await?;
        info.write().map_err(|e| e.to_string())?.access_jwt = Some(session.access_jwt);
        if let Some(did_doc) = session.did_doc {
            if let Some(pds_endpoint) = get_pds_endpoint(&did_doc) {
                info.write().map_err(|e| e.to_string())?.base_uri = pds_endpoint;
            }
        };
        Ok(Self {
            client,
            did: session.did,
        })
    }
    pub async fn create_post(
        &self,
        entry: &Entry,
        html: &HTML,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        let (text, facets) = text_and_facets(entry);
        let record = atrium_api::app::bsky::feed::post::Record {
            created_at: Datetime::now(),
            embed: Some(self.embed(entry, html).await?),
            entities: None,
            facets,
            labels: None,
            langs: Some(vec!["ja".parse().expect("invalid language")]),
            reply: None,
            tags: None,
            text,
        };
        Ok(self
            .client
            .service
            .com
            .atproto
            .repo
            .create_record(Input {
                collection: "app.bsky.feed.post".parse().expect("invalid collection"),
                record: Record::Known(KnownRecord::AppBskyFeedPost(Box::new(record))),
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
    ) -> Result<Union<RecordEmbedRefs>, Box<dyn std::error::Error>> {
        let thumb = if let Some(object) = html.opengraph.images.first() {
            let url = match object.url.parse() {
                Err(url::ParseError::RelativeUrlWithoutBase) => {
                    let mut base = entry.url.parse::<Url>()?;
                    base.set_path("/");
                    base.join(&object.url)
                }
                other => other,
            }?;
            let data = Fetch::Url(url).send().await?.bytes().await?;
            let uploaded = self
                .client
                .service
                .com
                .atproto
                .repo
                .upload_blob(data)
                .await?;
            Some(uploaded.blob)
        } else {
            None
        };
        Ok(Union::Refs(RecordEmbedRefs::AppBskyEmbedExternalMain(
            Box::new(external::Main {
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
            }),
        )))
    }
}

fn get_pds_endpoint(did_doc: &DidDocument) -> Option<String> {
    get_service_endpoint(did_doc, ("#atproto_pds", "AtprotoPersonalDataServer"))
}

fn get_service_endpoint(did_doc: &DidDocument, (id, r#type): (&str, &str)) -> Option<String> {
    let full_id = did_doc.id.clone() + id;
    if let Some(services) = &did_doc.service {
        let service = services
            .iter()
            .find(|service| service.id == id || service.id == full_id)?;
        if service.r#type == r#type && validate_url(&service.service_endpoint) {
            return Some(service.service_endpoint.clone());
        }
    }
    None
}

fn validate_url(url: &str) -> bool {
    if let Ok(uri) = url.parse::<Uri>() {
        if let Some(scheme) = uri.scheme() {
            if (scheme == "https" || scheme == "http") && uri.host().is_some() {
                return true;
            }
        }
    }
    false
}

fn text_and_facets(entry: &Entry) -> (String, Option<Vec<facet::Main>>) {
    let mut facets = Vec::new();
    let mut ret = if let Some(description) = &entry.description {
        format!("{description} / {}", entry.title)
    } else {
        entry.title.clone()
    };
    if !entry.tags.is_empty() {
        ret += "\n";
        for (i, tag) in entry.tags.iter().enumerate() {
            facets.push(facet::Main {
                features: vec![Union::Refs(facet::MainFeaturesItem::Tag(Box::new(
                    facet::Tag { tag: tag.clone() },
                )))],
                index: facet::ByteSlice {
                    byte_end: ret.len() + tag.len() + 1,
                    byte_start: ret.len(),
                },
            });
            ret += &format!("#{tag}");
            if i < entry.tags.len() - 1 {
                ret.push(' ');
            }
        }
    }
    (ret, Some(facets))
}
