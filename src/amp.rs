use amp_sdk::{AmpOptions, AssistantContent, StreamMessage, execute};
use anyhow::Result;
use futures::StreamExt;
use std::path::PathBuf;

pub async fn run_iteration(prompt: &str, cwd: &PathBuf) -> Result<String> {
    let cwd_str = cwd.canonicalize()?.to_string_lossy().to_string();

    let options = AmpOptions::builder()
        .cwd(&cwd_str)
        .dangerously_allow_all(true)
        .build();

    let mut stream = std::pin::pin!(execute(prompt, Some(options)));
    let mut output = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamMessage::System(msg)) => {
                eprintln!("[ramph] session: {}", msg.session_id);
            }
            Ok(StreamMessage::Assistant(msg)) => {
                for content in &msg.message.content {
                    match content {
                        AssistantContent::Text(text) => {
                            print!("{}", text.text);
                            output.push_str(&text.text);
                        }
                        AssistantContent::ToolUse(tool) => {
                            eprintln!("[ramph] using tool: {}", tool.name);
                        }
                    }
                }
            }
            Ok(StreamMessage::Result(msg)) => {
                eprintln!(
                    "[ramph] done: {}ms, {} turns",
                    msg.duration_ms, msg.num_turns
                );
                if msg.is_error {
                    anyhow::bail!("Amp error: {}", msg.error.unwrap_or_default());
                }
            }
            Err(e) => {
                eprintln!("[ramph] error: {}", e);
            }
            _ => {}
        }
    }

    Ok(output)
}
