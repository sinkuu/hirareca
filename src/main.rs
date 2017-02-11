#![feature(try_from, use_extern_macros, field_init_shorthand)]

extern crate xml;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate futures;
extern crate tokio_core;
extern crate tokio_curl;
extern crate curl;
extern crate url;
#[macro_use]
extern crate error_chain;
extern crate xdg;
extern crate toml;
#[macro_use]
extern crate log;
extern crate env_logger;

pub mod search;
pub mod rss;
mod server;

pub mod error {
    use xml;
    use tokio_curl;
    use curl;
    use serde_json;
    use toml;
    use std;

    error_chain! {
        foreign_links {
            Io(std::io::Error);
            XmlWriter(xml::writer::Error);
            SerdeJson(serde_json::Error);
            TomlSer(toml::ser::Error);
            TomlDe(toml::de::Error);
            TokioCurl(tokio_curl::PerformError);
            Curl(curl::Error);
            Utf8Error(::std::str::Utf8Error);
        }
    }
}

use error_chain::quick_main;
use std::fs::File;
use std::io::{Read, Write};
use std::env;
use error::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub api_key: String,
    pub custom_engine_id: String,
}

fn run() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        use log::LogLevelFilter;
        use env_logger::LogBuilder;
        let mut builder = LogBuilder::new();
        builder.filter(Some("hirareca"), LogLevelFilter::Info);
        builder.init()
    } else {
        env_logger::init()
    }.chain_err(|| "Failed to initialize logger")?;

    let xdg_dirs = xdg::BaseDirectories::with_prefix("hirareca")
        .chain_err(|| "Failed to obtain configuration directory")?;
    let config_path = xdg_dirs.place_config_file("config.toml")
        .chain_err(|| "Failed to create configuration directory")?;

    if !config_path.exists() {
        let mut f = File::create(&config_path)
            .chain_err(|| "Failed to create configuration file")?;
        write!(f,
r#"# API Key (see https://console.cloud.google.com/apis/dashboard)
api_key = ""
# Custom search engine id (see https://cse.google.com/cse/all)
custom_engine_id = """#)?;
        info!("Fill configuration file at {}", config_path.display());
        return Ok(());
    }

    let c = {
        let mut f = File::open(config_path).chain_err(|| "Failed to open configuration file")?;
        let mut v = vec![];
        f.read_to_end(&mut v)?;
        toml::de::from_slice(&v).chain_err(|| "Failed to parse configuration file")?
    };

    server::serve(c);
    Ok(())
}

quick_main!(run);
