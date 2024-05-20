use crate::STT_ADDRESS;
use kinode_process_lib::Request;
use stt_interface::STTRequest;

pub fn init_stt(key: &str) -> anyhow::Result<()> {
    let req = serde_json::to_vec(&STTRequest::RegisterApiKey(key.to_string()))?;
    let _ = Request::new()
        .target(STT_ADDRESS)
        .body(req)
        .send_and_await_response(5)??;

    Ok(())
}
