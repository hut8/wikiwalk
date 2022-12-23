#[cfg(feature = "tls-redirect")]
use std::convert::Infallible;
#[cfg(feature = "tls-redirect")]
use hyper::{http, Body, Request, Response, Uri};

#[cfg(not(feature = "tls-redirect"))]
pub async fn launch_tls_redirect() {
    println!("not launching tls redirect due to feature config");
}

#[cfg(feature = "tls-redirect")]
async fn tls_redirect(req: Request<Body>) -> http::Result<Response<Body>> {
    log::info!("plaintext request at {u}", u = req.uri());
    let host = match req.headers().get(http::header::HOST) {
        Some(host) => host.to_str().unwrap_or("wikipediaspeedrun.com"),
        None => {
            // just guess
            "wikipediaspeedrun.com"
        }
    };
    let destination = Uri::builder()
        .authority(host)
        .scheme("https")
        .path_and_query(req.uri().path_and_query().unwrap().to_owned())
        .build()
        .unwrap();
    log::info!("redirecting http to https: {}", destination.to_string());

    let res = Response::builder()
        .status(rocket::http::Status::MovedPermanently.code)
        .header(
            rocket::http::hyper::header::LOCATION,
            destination.to_string(),
        );
    res.body("".into())
}

#[cfg(feature = "tls-redirect")]
pub async fn launch_tls_redirect() {
    use std::net::SocketAddr;

    use rocket::http::hyper::{
        server::Server,
        service::{make_service_fn, service_fn},
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], 80));

    // A `Service` is needed for every connection, so this
    // creates one from our `hello_world` function.
    let make_svc = make_service_fn(|_conn| async {
        // service_fn converts our function into a `Service`
        Ok::<_, Infallible>(service_fn(tls_redirect))
    });

    //let server = Server::bind(&addr).serve(make_svc);
    let server = Server::bind(&addr).serve(make_svc);
    println!("launching tls redirect");
    // Run this server for... forever!
    if let Err(e) = server.await {
        panic!("tls redirect server error: {e}");
    }
}
