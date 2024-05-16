use std::collections::HashMap;

use kinode::process::standard::get_state;
use kinode_process_lib::http::send_response;
use kinode_process_lib::{
    await_message, call_init, get_blob, http, println, Address, Request, Response,
};
use oauth2::basic::BasicClient;
use serde::{Deserialize, Serialize};

wit_bindgen::generate!({
    path: "wit",
    world: "process",
});

// plan:
// flow, get goggle api key by signing in, and display it too.
// keep redirect_url there, have a default kinode to contact, otherwise use another.
// then spawn tg/groqAI.whisper?
// then mirror tg interface?
// then talk, implement context + basic gets for calendars.
// maybe choose calendar to use in the beginning or something?

// also, what about security with ze bot? setting a username?

call_init!(init);
fn init(our: Address) {
    println!("begin, our: {:?}", our);

    http::bind_http_path("/", false, false).unwrap();

    loop {
        match await_message() {
            Ok(m) => {}
            Err(e) => {
                println!("error: {:?}", e);
            }
        };
    }
}
