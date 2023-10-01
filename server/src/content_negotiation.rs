use actix_web::guard::GuardContext;
use actix_web::http::header;

pub fn accept_json_guard(ctx: &GuardContext) -> bool {
    // Either they explicitly ask for JSON, or they're using curl
    match ctx.header::<header::Accept>() {
        Some(hdr) => hdr.preference() == "application/json",
        None => match ctx.head().headers().get("user-agent") {
            Some(hdr) => hdr.to_str().unwrap_or_default().contains("curl"),
            None => false,
        },
    }
}

pub fn accept_html_guard(ctx: &GuardContext) -> bool {
    match ctx.header::<header::Accept>() {
        Some(hdr) => hdr.preference() == "text/html",
        None => false,
    }
}
