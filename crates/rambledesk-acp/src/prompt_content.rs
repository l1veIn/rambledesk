//! Typed mapping follows Codeg map_prompt_blocks in connection.rs at
//! 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1. RambleDesk validates inputs first
//! and uses negotiated capabilities without vendor-specific capability overrides.
use agent_client_protocol::schema::v1 as acp;
use rambledesk_core::{AgentPromptCapabilities, SessionPromptContent};

pub(crate) fn capabilities(capabilities: &acp::PromptCapabilities) -> AgentPromptCapabilities {
    AgentPromptCapabilities {
        image: capabilities.image,
        audio: capabilities.audio,
        embedded_context: capabilities.embedded_context,
        // Text and resource links are mandatory in the negotiated ACP baseline.
        resource_links: true,
    }
}

pub(crate) fn map(blocks: &[SessionPromptContent]) -> Vec<acp::ContentBlock> {
    blocks
        .iter()
        .map(|block| match block {
            SessionPromptContent::Text { text } => {
                acp::ContentBlock::Text(acp::TextContent::new(text.clone()))
            }
            SessionPromptContent::Image { mime_type, data } => {
                acp::ContentBlock::Image(acp::ImageContent::new(data.clone(), mime_type.clone()))
            }
            SessionPromptContent::ResourceLink {
                uri,
                name,
                mime_type,
            } => acp::ContentBlock::ResourceLink(
                acp::ResourceLink::new(name.clone(), uri.clone()).mime_type(mime_type.clone()),
            ),
            SessionPromptContent::Resource {
                uri,
                mime_type,
                text,
            } => acp::ContentBlock::Resource(acp::EmbeddedResource::new(
                acp::EmbeddedResourceResource::TextResourceContents(
                    acp::TextResourceContents::new(text.clone(), uri.clone())
                        .mime_type(mime_type.clone()),
                ),
            )),
        })
        .collect()
}
