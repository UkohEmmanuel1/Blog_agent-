use anyhow::{Context, Result};
use rig::{
    agent::AgentBuilder,
    prelude::*,
    providers::{anthropic::Anthropic, openai::OpenAI},
};

/// Run one bounded generation with a provider selected by the agent configuration.
/// API keys are read from the process environment and are never stored in agent rows.
pub async fn generate(
    provider: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let response = match provider.to_ascii_lowercase().as_str() {
        "openai" => {
            let client = OpenAI::from_env().context("OPENAI_API_KEY is required for OpenAI agents")?;
            let agent = AgentBuilder::new(client.completion(model.to_owned()))
                .preamble(system_prompt)
                .build();
            agent.prompt(user_prompt).await.context("OpenAI generation failed")?
        }
        "anthropic" => {
            let client = Anthropic::from_env().context("ANTHROPIC_API_KEY is required for Anthropic agents")?;
            let agent = AgentBuilder::new(client.completion(model.to_owned()))
                .preamble(system_prompt)
                .build();
            agent.prompt(user_prompt).await.context("Anthropic generation failed")?
        }
        other => anyhow::bail!("Unsupported LLM provider: {other}. Configure openai or anthropic."),
    };
    Ok(response)
}
