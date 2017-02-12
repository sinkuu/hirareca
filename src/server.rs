extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_minihttp;

use Config;
use url::Url;
use self::tokio_proto::TcpServer;
use self::tokio_minihttp::{Request, Response, Http};
use self::tokio_service::Service;
use tokio_curl::Session;
use futures::{future, Future, BoxFuture};
use std::convert::TryInto;
use xml::writer::EmitterConfig;
use std::io;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

struct SearchRssServer(Arc<Mutex<Session>>, Arc<Config>);

impl Service for SearchRssServer {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = BoxFuture<Response, io::Error>;

    fn call(&self, req: Request) -> Self::Future {
        let url = match Url::parse(&format!("http://dummy.example.com/{}", req.path())) {
            Ok(url) => url,
            Err(e) => {
                error!("parsing request url ({}): {}", req.path(), e);
                let mut res = Response::new();
                res.status_code(400, "Bad Request");
                return future::ok(res).boxed();
            }
        };

        let query: HashMap<_, _> = url.query_pairs().collect();

        let word = if let Some(q) = query.get("q") {
            q.to_string()
        } else {
            let mut res = Response::new();
            res.status_code(400, "Bad Request");
            return future::ok(res).boxed();
        };

        info!("search request: '{}'", word);

        let session = self.0.lock().unwrap().clone();

        ::search::list(word.to_string(), session, self.1.clone())
            .and_then(move |list| -> ::error::Result<Response> {
                debug!("responding {} items for '{}' search", list.items.len(), word);

                let mut output = vec![];
                {
                    let mut writer = EmitterConfig::new()
                        .perform_indent(true)
                        .create_writer(&mut output);

                    ::rss::write_rss(&mut writer,
                        list.try_into().map_err(|()| ::error::Error::from_kind("missing fields".into()))?)?;
                }

                let mut resp = Response::new();
                resp.header("Content-Type", "text/xml")
                    .body(::std::str::from_utf8(&output)?);
                Ok(resp)
            })
            .then(|res| -> io::Result<Response> {
                match res {
                    Ok(resp) => Ok(resp),
                    Err(e) => {
                        error!("{}", e);
                        let mut res = Response::new();
                        res.status_code(500, "Internal Error");
                        Ok(res)
                    }
                }
            })
            .boxed()
    }
}

pub fn serve(cfg: ::Config) {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), cfg.port);
    let srv = TcpServer::new(Http, addr);
    let cfg = Arc::new(cfg);
    srv.with_handle(move |handle| {
        let session = Arc::new(Mutex::new(Session::new(handle.clone())));
        let cfg = cfg.clone();
        (move || Ok(SearchRssServer(session.clone(), cfg.clone())))
    });
}
