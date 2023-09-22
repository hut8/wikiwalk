use actix_web::{guard::fn_guard, http::header};
use actix_web::guard::GuardContext;


fn accept_json_guard(ctx: &GuardContext) {
    fn_guard(|ctx| {
        match ctx.header::<header::Accept>() {
            Some(hdr) => hdr.preference() == "application/json",
            None => false
        }
    })
}