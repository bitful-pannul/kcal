use crate::LLM_ADDRESS;
use kinode_process_lib::Request;
use llm_interface::openai::{
    ChatRequestBuilder, LLMRequest, LLMResponse, MessageBuilder, RegisterApiKeyRequest,
};

pub fn init_groq(key: &str) -> anyhow::Result<()> {
    let req = serde_json::to_vec(&llm_interface::openai::LLMRequest::RegisterGroqApiKey(
        RegisterApiKeyRequest {
            api_key: key.to_string(),
        },
    ))?;
    let _ = Request::new()
        .target(LLM_ADDRESS)
        .body(req)
        .send_and_await_response(5)??;
    Ok(())
}

pub fn get_groq_answer(text: &str) -> anyhow::Result<String> {
    let request = ChatRequestBuilder::default()
        .model("llama3-70b-8192".to_string())
        .messages(vec![MessageBuilder::default()
            .role("user".to_string())
            .content(text.to_string())
            .build()?])
        .build()?;
    let request = serde_json::to_vec(&LLMRequest::GroqChat(request))?;
    let response = Request::to(LLM_ADDRESS)
        .body(request)
        .send_and_await_response(30)??;
    let LLMResponse::Chat(chat) = serde_json::from_slice(response.body())? else {
        return Err(anyhow::anyhow!("Failed to parse LLM response"));
    };
    Ok(chat.choices[0].message.content.clone())
}
