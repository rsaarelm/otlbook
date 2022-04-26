use std::str::FromStr;

use crate::{html::Html, resolver::Command};
use base::Collection;
use rouille::{Request, Response};

mod html;
mod resolver;

pub fn run(port: u32, collection: Collection) -> ! {
    let addr = format!("localhost:{}", port);
    println!("Starting server at http://{}", addr);
    rouille::start_server(addr, move |request| {
        match Command::from_str(&request.url()) {
            Ok(Command::ViewArticle(a)) => {
                // The crappiest selector
                for section in collection.iter() {
                    if section.title() == a {
                        return Response::html(format!("{}", Html(section)));
                    }
                }
                Response::empty_404()
            }
            Ok(cmd) => Response::text(format!("TODO: {:?}", cmd)),
            Err(_) => Response::empty_404(),
        }
    })
}
