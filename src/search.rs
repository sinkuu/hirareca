use Config;
use error::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde_json;
use futures::{Async, Poll, Future};
use tokio_curl::{Session, Perform};
use curl::easy::Easy;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct List {
    pub queries: HashMap<String, Vec<Query>>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub title: Option<String>,
    #[serde(rename = "totalResults")]
    pub total_results: Option<String>,
    #[serde(rename = "searchTerms")]
    pub search_terms: Option<String>,
    #[serde(rename = "startIndex")]
    pub start_index: Option<u32>,
    pub count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub title: String,
    pub link: String,
    pub snippet: String,
}

pub enum ListFuture {
    Configuring { session: Session, word: String, config: Arc<Config> },
    Performing {
        perform: Perform,
        dump: Arc<Mutex<Vec<u8>>>,
    },
    /// Done operation or failed with error.
    Done,
}

impl Future for ListFuture {
    type Item = List;
    type Error = Error;

    fn poll(&mut self) -> Poll<List, Error> {
        match ::std::mem::replace(self, ListFuture::Done) {
            ListFuture::Done => {
                panic!("Polling future which has been resolved or failed");
            }
            ListFuture::Configuring { session, word, config } => {
                let dump = Arc::new(Mutex::new(Vec::new()));

                let mut url = Url::parse("https://www.googleapis.com/customsearch/v1")
                    .unwrap_or_else(|_| unreachable!());

                url.query_pairs_mut()
                    .append_pair("key", &config.api_key)
                    .append_pair("cx", &config.custom_engine_id)
                    .append_pair("q", &word);
                debug!("requesting url {}", url);

                let mut req = Easy::new();
                req.get(true)?;
                req.url(url.as_str())?;
                {
                    let dump = dump.clone();
                    req.write_function(move |data| {
                            if let Ok(mut dump) = dump.lock() {
                                dump.extend(data);
                            }
                            Ok(data.len())
                        })?;
                }

                *self = ListFuture::Performing {
                    perform: session.perform(req),
                    dump,
                };

                self.poll()
            }

            ListFuture::Performing { mut perform, dump } => {
                let mut res = if let Async::Ready(res) = perform.poll()? {
                    res
                } else {
                    *self = ListFuture::Performing {
                        perform, dump,
                    };
                    return Ok(Async::NotReady);
                };
                let code = res.response_code().chain_err(|| "Failed to obtain response code")?;
                if code == 200 {
                    // this will drop old write_function, hence dropping `Arc<Mutex<_>>`
                    res.write_function(|_| unreachable!())?;
                    let dump = Arc::try_unwrap(dump)
                        .map_err(|_| Error::from_kind("Arc should have been dropped".into()))?;
                    let dump = dump.into_inner()
                        .map_err(|_| Error::from_kind("Mutex guards should have been dropped".into()))?;
                    Ok(Async::Ready(serde_json::from_slice(&dump)?))
                } else {
                    let msg = format!("curl failed, response code = {}", code);
                    debug!("{}", msg);
                    Err(msg.into())
                }
            }
        }
    }
}

pub fn list(word: String, session: Session, config: Arc<Config>) -> ListFuture {
    ListFuture::Configuring {
        session,
        word,
        config,
    }
}
