use std::convert::Infallible;

use hyper::{Body, Request, Response, Uri, http};

#[cfg(not(redirect_tls))]
pub async fn launch_tls_redirect() {
  log::debug!("not launching tls redirect due to feature config");
}

async fn tls_redirect(req: Request<Body>) -> http::Result<Response<Body>> {
    let plain_uri = req.uri().to_owned().into_parts();
    let authority = plain_uri.authority.unwrap();
    let host = authority.host();
    let destination = Uri::builder()
        .authority(host)
        .scheme("https")
        .build()
        .unwrap();
    log::debug!("redirecting http to https: {}", destination.to_string());

    let res = Response::builder()
        .status(rocket::http::Status::MovedPermanently.code)
        .header(
            rocket::http::hyper::header::LOCATION,
            destination.to_string(),
        );
    res.body("".into())
}

#[cfg(redirect_tls)]
pub async fn launch_tls_redirect() {
    use std::net::SocketAddr;

    use rocket::http::hyper::{
        server::Server,
        service::{make_service_fn, service_fn},
    };

    let addr = SocketAddr::from(([0,0,0,0],80));

    // A `Service` is needed for every connection, so this
    // creates one from our `hello_world` function.
    let make_svc = make_service_fn(|_conn| async {
        // service_fn converts our function into a `Service`
        Ok::<_, Infallible>(service_fn(tls_redirect))
    });

    //let server = Server::bind(&addr).serve(make_svc);
    let server = Server::bind(&addr).serve(make_svc);

    // Run this server for... forever!
    if let Err(e) = server.await {
        eprintln!("tls redirect server error: {e}");
    }
}
