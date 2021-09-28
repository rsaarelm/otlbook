use async_std::{prelude::*, task};
use base::Result;
use lazy_static::lazy_static;
use tide::{prelude::*, Request};

async fn run_server(port: u32) -> Result<()> {
    let mut app = tide::new();

    // Redirect root to FrontPage article
    app.at("/").get(|_request: Request<()>| async move {
        let ret: tide::Result = Ok(tide::Redirect::new("/a/FrontPage").into());
        ret
    });

    app.at("/a/:title").get(|req: Request<()>| async move {
        Ok(format!("Hello, world! {:?}", req.param("title")))
    });

    let addr = format!("127.0.0.1:{}", port);
    println!("Starting server at http://{}", addr);
    app.listen(addr).await?;
    Ok(())
}

pub fn run(port: u32) -> Result<()> {
    let fut = run_server(port);
    task::block_on(fut)
}
