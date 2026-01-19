use amp_sdk::{execute, AmpOptions, AssistantContent, StreamMessage};
use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ramph", about = "Ralph workflow runner using Amp")]
struct Cli {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(short, long, default_value = "prd.json")]
    prd: PathBuf,

    #[arg(short, long, default_value = "progress.txt")]
    progress: PathBuf,

    #[arg(short, long, default_value = "prompt.md")]
    prompt: PathBuf,

    #[arg(long, default_value_t = 25)]
    max_iterations: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Prd {
    #[serde(rename = "branchName")]
    branch_name: String,
    stories: Vec<Story>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Story {
    id: String,
    title: String,
    description: String,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    passes: bool,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
}

fn load_prd(path: &PathBuf) -> Result<Prd> {
    let content = fs::read_to_string(path).context("Failed to read prd.json")?;
    serde_json::from_str(&content).context("Failed to parse prd.json")
}

#[allow(dead_code)]
fn save_prd(path: &PathBuf, prd: &Prd) -> Result<()> {
    let content = serde_json::to_string_pretty(prd)?;
    fs::write(path, content).context("Failed to write prd.json")
}

fn load_prompt(path: &PathBuf) -> Result<String> {
    fs::read_to_string(path).context("Failed to read prompt.md")
}

fn load_progress(path: &PathBuf) -> Result<String> {
    if path.exists() {
        fs::read_to_string(path).context("Failed to read progress.txt")
    } else {
        Ok(String::new())
    }
}

fn append_progress(path: &PathBuf, entry: &str) -> Result<()> {
    let mut content = load_progress(path).unwrap_or_default();
    content.push_str(entry);
    content.push('\n');
    fs::write(path, content).context("Failed to write progress.txt")
}

fn get_next_story(prd: &Prd) -> Option<&Story> {
    prd.stories
        .iter()
        .filter(|s| !s.passes)
        .min_by_key(|s| s.priority)
}

fn build_iteration_prompt(base_prompt: &str, story: &Story, progress: &str) -> String {
    format!(
        r#"{base_prompt}

## Current Task

**Story ID:** {id}
**Title:** {title}
**Description:** {description}

### Acceptance Criteria
{criteria}

## Previous Learnings
{progress}

## Instructions

1. Implement this story
2. Run typecheck and tests
3. If passing, commit with message: "feat({id}): {title}"
4. Mark the story as done by setting `passes: true` in prd.json
5. Append learnings to progress.txt
6. If you discover reusable patterns, update AGENTS.md
"#,
        id = story.id,
        title = story.title,
        description = story.description,
        criteria = story
            .acceptance_criteria
            .iter()
            .map(|c| format!("- {c}"))
            .collect::<Vec<_>>()
            .join("\n"),
        progress = if progress.is_empty() {
            "(none yet)".to_string()
        } else {
            progress.to_string()
        }
    )
}

async fn run_iteration(prompt: &str, cwd: &PathBuf) -> Result<String> {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let prd_path = cli.cwd.join(&cli.prd);
    let progress_path = cli.cwd.join(&cli.progress);
    let prompt_path = cli.cwd.join(&cli.prompt);

    let base_prompt = load_prompt(&prompt_path)?;

    for iteration in 1..=cli.max_iterations {
        let prd = load_prd(&prd_path)?;

        let Some(story) = get_next_story(&prd) else {
            eprintln!("[ramph] All stories complete!");
            break;
        };

        let story_id = story.id.clone();
        eprintln!(
            "\n[ramph] === Iteration {}/{} ===",
            iteration, cli.max_iterations
        );
        eprintln!("[ramph] Working on: {} - {}", story.id, story.title);

        let progress = load_progress(&progress_path)?;
        let prompt = build_iteration_prompt(&base_prompt, story, &progress);

        match run_iteration(&prompt, &cli.cwd).await {
            Ok(_output) => {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                append_progress(
                    &progress_path,
                    &format!("\n## [{timestamp}] Completed: {story_id}\n"),
                )?;
            }
            Err(e) => {
                eprintln!("[ramph] Iteration failed: {}", e);
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                append_progress(
                    &progress_path,
                    &format!("\n## [{timestamp}] Failed: {story_id}\nError: {e}\n"),
                )?;
            }
        }
    }

    Ok(())
}
