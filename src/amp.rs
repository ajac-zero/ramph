use amp_sdk::{execute, AmpOptions, AssistantContent, StreamMessage};
use anyhow::Result;
use futures::StreamExt;
use indicatif::ProgressBar;
use std::path::PathBuf;

use crate::output;

pub async fn run_iteration(prompt: &str, cwd: &PathBuf, spinner: Option<&ProgressBar>) -> Result<String> {
    let cwd_str = cwd.canonicalize()?.to_string_lossy().to_string();

    let options = AmpOptions::builder()
        .cwd(&cwd_str)
        .dangerously_allow_all(true)
        .build();

    let mut stream = std::pin::pin!(execute(prompt, Some(options)));
    let mut output_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamMessage::System(msg)) => {
                output::verbose(&format!("session: {}", msg.session_id));
            }
            Ok(StreamMessage::Assistant(msg)) => {
                for content in &msg.message.content {
                    match content {
                        AssistantContent::Text(text) => {
                            if !output::is_quiet() {
                                print!("{}", text.text);
                            }
                            output_text.push_str(&text.text);
                        }
                        AssistantContent::ToolUse(tool) => {
                            if let Some(s) = spinner {
                                s.set_message(format!("Using tool: {}...", tool.name));
                            }
                            output::verbose(&format!("using tool: {}", tool.name));
                        }
                    }
                }
            }
            Ok(StreamMessage::Result(msg)) => {
                output::verbose(&format!(
                    "done: {}ms, {} turns",
                    msg.duration_ms, msg.num_turns
                ));
                if msg.is_error {
                    anyhow::bail!("Amp error: {}", msg.error.unwrap_or_default());
                }
            }
            Err(e) => {
                output::error(&format!("stream error: {}", e));
            }
            _ => {}
        }
    }

    Ok(output_text)
}
