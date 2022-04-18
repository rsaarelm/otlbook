use rouille::{Request, Response};

mod resolver;

/*
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
*/

fn server(_request: &Request) -> Response {
    Response::text("hello, world")
}

pub fn run(port: u32) -> ! {
    let addr = format!("0.0.0.0:{}", port);
    println!("Starting server at http://{}", addr);
    rouille::start_server(addr, server)
}
