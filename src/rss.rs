use search;
use error::*;
use std::io::Write;
use std::convert::TryFrom;
use xml::common::XmlVersion;
use xml::name::Name;
use xml::writer::{EventWriter, XmlEvent};

#[derive(Debug)]
pub struct Response {
    pub title: String,
    pub link: String,
    pub description: String,
    pub total_results: u64,
    pub start_index: u32,
    pub items_per_page: u32,
    pub items: Vec<Item>,
}

impl TryFrom<search::List> for Response {
    type Err = ();

    fn try_from(s: search::List) -> ::std::result::Result<Response, ()> {
        let search::List { mut queries, items } = s;

        let request = queries.remove("request")
            .ok_or(())?
            .into_iter()
            .next()
            .ok_or(())?;

        let next_page = queries.remove("nextPage")
            .ok_or(())?
            .into_iter()
            .next()
            .ok_or(())?;

        let items = items.into_iter()
            .map(|item| {
                Ok(Item {
                    title: item.title,
                    link: item.link,
                    description: item.snippet,
                })
            })
            .collect::<::std::result::Result<Vec<Item>, _>>()?;

        Ok(Response {
            title: request.title.clone().ok_or(())?,
            description: request.title.ok_or(())?,
            link: format!("https://www.googleapis.com/customsearch/v1?q={}",
                          request.search_terms.ok_or(())?),
            total_results: request.total_results.ok_or(())?.parse::<u64>().map_err(|_| ())?,
            start_index: request.start_index.ok_or(())?,
            items_per_page: (next_page.start_index.ok_or(())? - request.start_index.ok_or(())?),
            items: items,
        })
    }
}

#[derive(Debug)]
pub struct Item {
    pub title: String,
    pub link: String,
    pub description: String,
}

fn write_value<'a, W: Write, N: Into<Name<'a>>>(w: &mut EventWriter<W>, name: N, value: &str) {
    w.write(XmlEvent::start_element(name)).unwrap();
    w.write(XmlEvent::characters(value)).unwrap();
    w.write(XmlEvent::end_element()).unwrap();
}

pub fn write_rss<W: Write>(w: &mut EventWriter<W>, resp: Response) -> Result<()> {
    w.write(XmlEvent::StartDocument {
            version: XmlVersion::Version10,
            encoding: Some("UTF-8"),
            standalone: None,
        })?;

    w.write(XmlEvent::start_element("rss")
            .attr("version", "2.0")
            .attr(("xmlns", "opensearch"),
                  "http://a9.com/-/spec/opensearch/1.1/"))?;

    w.write(XmlEvent::start_element("channel"))?;

    write_value(w, "title", &resp.title);
    write_value(w, "link", &resp.link);
    write_value(w, "description", &resp.description);
    write_value(w,
                ("opensearch", "totalResults"),
                &resp.total_results.to_string());
    write_value(w,
                ("opensearch", "startIndex"),
                &resp.start_index.to_string());
    write_value(w,
                ("opensearch", "itemsPerPage"),
                &resp.items_per_page.to_string());

    for item in resp.items {
        w.write(XmlEvent::start_element("item"))?;

        write_value(w, "title", &item.title);
        write_value(w, "link", &item.link);
        write_value(w, "description", &item.description);

        w.write(XmlEvent::end_element())?;
    }

    // channel
    w.write(XmlEvent::end_element())?;

    // rss
    w.write(XmlEvent::end_element())?;

    Ok(())
}
