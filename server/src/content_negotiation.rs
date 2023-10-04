use actix_web::guard::GuardContext;
use actix_web::http::header;

pub fn accept_json_guard(ctx: &GuardContext) -> bool {
    // This makes JSON the default if no Accept header is present.
    match ctx.header::<header::Accept>() {
        Some(hdr) => hdr.preference() == "application/json",
        None => true,
    }
}

pub fn accept_html_guard(ctx: &GuardContext) -> bool {
    match ctx.header::<header::Accept>() {
        Some(hdr) => hdr.preference() == "text/html",
        None => false,
    }
}
