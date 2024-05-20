use crate::{STT_ADDRESS, TG_ADDRESS};
use frankenstein::GetFileParams;
use frankenstein::{ChatId, Message as TgMessage, SendMessageParams, UpdateContent};
use kinode_process_lib::{get_blob, Message, Request};
use stt_interface::STTRequest;
use stt_interface::STTResponse;
use telegram_interface::*;

pub fn init_tg(key: &str) -> anyhow::Result<()> {
    let init_req = TgInitialize {
        token: key.to_string(),
        params: None,
    };

    let req = serde_json::to_vec(&TgRequest::RegisterApiKey(init_req))?;

    let response = Request::to(TG_ADDRESS)
        .body(req)
        .send_and_await_response(3)??;
    let TgResponse::Ok = serde_json::from_slice(response.body())? else {
        return Err(anyhow::anyhow!("Failed to parse init response"));
    };
    Ok(())
}

pub fn send_bot_message(text: &str, id: i64) -> anyhow::Result<TgMessage> {
    let params = SendMessageParams::builder()
        .chat_id(ChatId::Integer(id))
        .text(text)
        .build();
    let send_message_request = serde_json::to_vec(&TgRequest::SendMessage(params))?;
    let response = Request::to(TG_ADDRESS)
        .body(send_message_request)
        .send_and_await_response(30)??;
    let TgResponse::SendMessage(message) = serde_json::from_slice(response.body())? else {
        return Err(anyhow::anyhow!("Failed to send message"));
    };
    Ok(message)
}

pub fn get_text(audio: Vec<u8>) -> anyhow::Result<String> {
    let stt_request = serde_json::to_vec(&STTRequest::OpenaiTranscribe(audio))?;
    let response = Request::to(STT_ADDRESS)
        .body(stt_request)
        .send_and_await_response(3)??;
    let STTResponse::OpenaiTranscribed(text) = serde_json::from_slice(response.body())? else {
        return Err(anyhow::anyhow!("Failed to parse STT response"));
    };
    Ok(text)
}

pub fn get_file(file_id: &str) -> anyhow::Result<Vec<u8>> {
    let get_file_params = GetFileParams::builder().file_id(file_id).build();
    let tg_request = serde_json::to_vec(&TgRequest::GetFile(get_file_params))?;
    let _ = Request::to(TG_ADDRESS)
        .body(tg_request)
        .send_and_await_response(10)??;
    if let Some(blob) = get_blob() {
        return Ok(blob.bytes);
    }
    Err(anyhow::anyhow!("Failed to get file"))
}
pub fn get_last_tg_msg(message: &Message) -> Option<TgMessage> {
    let Ok(TgResponse::Update(tg_update)) = serde_json::from_slice(message.body()) else {
        return None;
    };
    let update = tg_update.updates.last()?;
    let msg = match &update.content {
        UpdateContent::Message(msg) | UpdateContent::ChannelPost(msg) => msg,
        _ => {
            return None;
        }
    };
    Some(msg.clone())
}

pub fn subscribe() -> anyhow::Result<()> {
    let subscribe_request = serde_json::to_vec(&TgRequest::Subscribe)?;
    let result = Request::to(TG_ADDRESS)
        .body(subscribe_request)
        .send_and_await_response(3)??;
    let TgResponse::Ok = serde_json::from_slice::<TgResponse>(result.body())? else {
        return Err(anyhow::anyhow!("Failed to parse subscription response"));
    };
    Ok(())
}
