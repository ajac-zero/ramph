use amp_sdk::{AmpOptions, AssistantContent, StreamMessage, execute};
use anyhow::{Context, Result};
use chrono::Local;
use clap::{Args, Parser, Subcommand};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ramph", about = "Ralph workflow runner using Amp")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute stories from an existing PRD
    Run(RunArgs),
    /// Interactively create a new PRD with AI assistance
    Plan(PlanArgs),
}

#[derive(Args)]
struct RunArgs {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(long, default_value = "prd.json")]
    prd: PathBuf,

    #[arg(long, default_value = "progress.txt")]
    progress: PathBuf,

    #[arg(long, default_value = "prompt.md")]
    prompt: PathBuf,

    #[arg(long, default_value_t = 25)]
    max_iterations: usize,
}

#[derive(Args)]
struct PlanArgs {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(short, long, default_value = "prd.json")]
    output: PathBuf,

    /// Initial project description (optional, can be provided interactively)
    #[arg(short, long)]
    description: Option<String>,

    /// Overwrite existing PRD file if it exists
    #[arg(long, default_value_t = false)]
    force: bool,
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

const PLANNING_PROMPT_TEMPLATE: &str = r#"You are an AI planning assistant helping a developer break down their project into actionable user stories for a Product Requirements Document (PRD).

## Your Role

1. Have a natural conversation to understand what they want to build
2. Ask clarifying questions about:
   - Core functionality and features
   - Technical constraints or preferences
   - Success criteria
   - Edge cases and error handling
   - Testing requirements

3. Propose a breakdown into stories with:
   - Clear, focused scope (each story should be completable in one session)
   - Specific acceptance criteria
   - Logical priority ordering
   - Unique story IDs (format: STORY-001, STORY-002, etc.)

4. Refine based on their feedback

## Guidelines

- Keep stories small and focused (prefer 8-12 stories over 2-3 mega-stories)
- Order by dependency (foundational work first)
- Include setup/infrastructure stories
- Include testing/validation stories
- Be specific in acceptance criteria
- Use technical language appropriate for developers

## Conversation Style

- Be concise but thorough
- Ask one question at a time or group related questions
- Confirm understanding before proposing stories
- Be open to iteration and refinement

{initial_context}

Begin by understanding what the user wants to build."#;

const EXTRACTION_PROMPT: &str = r#"You are a structured data extraction assistant. Review the conversation history below and extract a valid PRD (Product Requirements Document) in JSON format.

## Required JSON Structure

{{
  "branchName": "feature/descriptive-name",
  "stories": [
    {{
      "id": "STORY-001",
      "title": "Brief title",
      "description": "Detailed description of what needs to be done",
      "priority": 1,
      "passes": false,
      "acceptance_criteria": [
        "Specific criterion 1",
        "Specific criterion 2"
      ]
    }}
  ]
}}

## Requirements

1. Generate a meaningful branch name based on the project
2. Extract all agreed-upon stories from the conversation
3. Priority: number from 1 (highest) to N (lowest), ordered by implementation sequence
4. Set all "passes" to false (work hasn't started yet)
5. Acceptance criteria should be specific, testable conditions

## Output Format

Output ONLY the JSON, no markdown fences, no explanation text.
Start with {{ and end with }}

## Conversation History

{conversation_history}"#;

fn load_prd(path: &PathBuf) -> Result<Prd> {
    let content = fs::read_to_string(path).context("Failed to read prd.json")?;
    serde_json::from_str(&content).context("Failed to parse prd.json")
}

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

async fn run_command(args: RunArgs) -> Result<()> {
    let prd_path = args.cwd.join(&args.prd);
    let progress_path = args.cwd.join(&args.progress);
    let prompt_path = args.cwd.join(&args.prompt);

    let base_prompt = load_prompt(&prompt_path)?;

    for iteration in 1..=args.max_iterations {
        let prd = load_prd(&prd_path)?;

        let Some(story) = get_next_story(&prd) else {
            eprintln!("[ramph] All stories complete!");
            break;
        };

        let story_id = story.id.clone();
        eprintln!(
            "\n[ramph] === Iteration {}/{} ===",
            iteration, args.max_iterations
        );
        eprintln!("[ramph] Working on: {} - {}", story.id, story.title);

        let progress = load_progress(&progress_path)?;
        let prompt = build_iteration_prompt(&base_prompt, story, &progress);

        match run_iteration(&prompt, &args.cwd).await {
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

fn build_planning_prompt(initial_description: Option<String>) -> String {
    let initial_context = match initial_description {
        Some(desc) => format!(
            "\n## Initial Project Description\n\n{}\n\nStart by clarifying any questions about this description.",
            desc
        ),
        None => String::new(),
    };

    PLANNING_PROMPT_TEMPLATE.replace("{initial_context}", &initial_context)
}

fn build_extraction_prompt(conversation_history: &str) -> String {
    EXTRACTION_PROMPT.replace("{conversation_history}", conversation_history)
}

async fn run_planning_session(
    initial_description: Option<String>,
    cwd: &PathBuf,
) -> Result<String> {
    let prompt = build_planning_prompt(initial_description);

    eprintln!("\n[ramph] Starting planning conversation...");
    eprintln!("[ramph] The AI agent will help you break down your project into stories.\n");

    // Run the conversational agent
    let conversation = run_iteration(&prompt, cwd).await?;

    Ok(conversation)
}

fn clean_json_response(response: &str) -> Result<String> {
    let trimmed = response.trim();

    // Remove markdown code fences if present
    let without_fences = if trimmed.starts_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() > 2 {
            // Skip first and last lines (fences)
            lines[1..lines.len() - 1].join("\n")
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    // Find JSON boundaries
    let start = without_fences
        .find('{')
        .context("No opening brace found in response")?;
    let end = without_fences
        .rfind('}')
        .context("No closing brace found in response")?;

    Ok(without_fences[start..=end].to_string())
}

async fn extract_prd_from_conversation(conversation: &str, cwd: &PathBuf) -> Result<Prd> {
    eprintln!("\n[ramph] Generating structured PRD from conversation...\n");

    let extraction_prompt = build_extraction_prompt(conversation);
    let json_response = run_iteration(&extraction_prompt, cwd).await?;

    // Clean the response - remove markdown fences if present
    let cleaned = clean_json_response(&json_response)
        .context("Failed to extract JSON from agent response")?;

    // Parse and validate
    let prd: Prd = serde_json::from_str(&cleaned).context("Failed to parse generated PRD JSON")?;

    Ok(prd)
}

fn validate_prd(prd: &Prd) -> Result<()> {
    anyhow::ensure!(!prd.branch_name.is_empty(), "Branch name cannot be empty");
    anyhow::ensure!(
        !prd.stories.is_empty(),
        "PRD must contain at least one story"
    );

    let mut seen_ids = std::collections::HashSet::new();

    for (idx, story) in prd.stories.iter().enumerate() {
        anyhow::ensure!(!story.id.is_empty(), "Story at index {} has empty ID", idx);
        anyhow::ensure!(
            !story.title.is_empty(),
            "Story {} has empty title",
            story.id
        );
        anyhow::ensure!(
            !story.description.is_empty(),
            "Story {} has empty description",
            story.id
        );
        anyhow::ensure!(
            story.priority > 0,
            "Story {} has invalid priority: {}",
            story.id,
            story.priority
        );
        anyhow::ensure!(
            !story.acceptance_criteria.is_empty(),
            "Story {} has no acceptance criteria",
            story.id
        );
        anyhow::ensure!(
            seen_ids.insert(&story.id),
            "Duplicate story ID: {}",
            story.id
        );
    }

    Ok(())
}

fn display_prd_summary(prd: &Prd) {
    eprintln!("\n[ramph] === PRD Summary ===");
    eprintln!("Branch: {}", prd.branch_name);
    eprintln!("Stories: {}\n", prd.stories.len());

    for story in &prd.stories {
        eprintln!("  {} [P{}]: {}", story.id, story.priority, story.title);
        eprintln!("    Criteria: {} items", story.acceptance_criteria.len());
    }
    eprintln!();
}

fn check_output_file(path: &PathBuf, force: bool) -> Result<()> {
    if path.exists() && !force {
        anyhow::bail!(
            "PRD file already exists at {}. Use --force to overwrite.",
            path.display()
        );
    }
    Ok(())
}

async fn run_plan_command(args: PlanArgs) -> Result<()> {
    let output_path = args.cwd.join(&args.output);

    // Check if file exists
    check_output_file(&output_path, args.force).context("Output file validation failed")?;

    // Run planning session
    let conversation = run_planning_session(args.description, &args.cwd)
        .await
        .context("Planning session failed")?;

    eprintln!("\n[ramph] Planning conversation complete!");

    // Extract PRD from conversation
    let prd = extract_prd_from_conversation(&conversation, &args.cwd)
        .await
        .context("Failed to extract PRD from conversation")?;

    // Validate PRD
    validate_prd(&prd).context("PRD validation failed")?;

    // Display summary and confirm
    display_prd_summary(&prd);

    eprint!(
        "[ramph] Save this PRD to {}? (y/n): ",
        output_path.display()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        eprintln!("[ramph] PRD not saved. Exiting.");
        return Ok(());
    }

    // Save PRD
    save_prd(&output_path, &prd).context("Failed to save PRD")?;
    eprintln!(
        "[ramph] PRD saved successfully to {}!",
        output_path.display()
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => run_command(args).await,
        Commands::Plan(args) => run_plan_command(args).await,
    }
}
