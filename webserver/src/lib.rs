use std::str::FromStr;

use crate::resolver::Command;
use base::Collection;
use rouille::{Request, Response};

mod resolver;

fn server(request: &Request) -> Response {
    if let Ok(cmd) = Command::from_str(&request.url()) {
        Response::text(format!("{:?}", cmd))
    } else {
        Response::empty_404()
    }
}

pub fn run(port: u32, collection: Collection) -> ! {
    let addr = format!("localhost:{}", port);
    println!("Starting server at http://{}", addr);
    rouille::start_server(addr, server)
}
