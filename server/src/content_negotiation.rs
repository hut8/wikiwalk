use actix_web::{guard::fn_guard, http::header};
use actix_web::guard::{GuardContext, Guard};


fn accept_json_guard(ctx: &GuardContext) -> impl Guard {
    fn_guard(|ctx| {
        match ctx.header::<header::Accept>() {
            Some(hdr) => hdr.preference() == "application/json",
            None => false
        }
    })
}

fn accept_html_guard(ctx: &GuardContext) -> impl Guard {
  fn_guard(|ctx| {
      match ctx.header::<header::Accept>() {
          Some(hdr) => hdr.preference() == "text/html",
          None => false
      }
  })
}
