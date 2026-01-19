use anyhow::{Context, Result};
use chrono::Local;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::amp::run_iteration;
use crate::prompts::*;
use crate::types::*;

pub async fn run_command(
    cwd: PathBuf,
    prd: PathBuf,
    progress: Option<PathBuf>,
    prompt: Option<PathBuf>,
    max_iterations: usize,
) -> Result<()> {
    let prd_path = cwd.join(&prd);
    let progress_path = match &progress {
        Some(p) => cwd.join(p),
        None => cwd.join("progress.txt"),
    };

    let base_prompt = match prompt.as_ref() {
        Some(p) => load_prompt(Some(&cwd.join(p)))?,
        None => load_prompt(None)?,
    };

    for iteration in 1..=max_iterations {
        let prd = load_prd(&prd_path)?;

        let Some(story) = prd.get_next_story() else {
            eprintln!("[ramph] All stories complete!");
            break;
        };

        let story_id = story.id.clone();
        eprintln!(
            "\n[ramph] === Iteration {}/{} ===",
            iteration, max_iterations
        );
        eprintln!("[ramph] Working on: {} - {}", story.id, story.title);

        let progress = load_progress(&progress_path)?;
        let prompt = build_iteration_prompt(&base_prompt, story, &progress);

        match run_iteration(&prompt, &cwd).await {
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

pub async fn run_plan_command(
    cwd: PathBuf,
    output: PathBuf,
    description: Option<String>,
    force: bool,
) -> Result<()> {
    let output_path = cwd.join(&output);

    check_output_file(&output_path, force).context("Output file validation failed")?;

    // Run planning session
    let prompt = build_planning_prompt(description);
    eprintln!("\n[ramph] Starting planning conversation...");
    eprintln!("[ramph] The AI agent will help you break down your project into stories.\n");

    let conversation = run_iteration(&prompt, &cwd)
        .await
        .context("Planning session failed")?;

    eprintln!("\n[ramph] Planning conversation complete!");

    // Extract PRD from conversation
    eprintln!("\n[ramph] Generating structured PRD from conversation...\n");
    let extraction_prompt = build_extraction_prompt(&conversation);
    let json_response = run_iteration(&extraction_prompt, &cwd).await?;

    let cleaned = clean_json_response(&json_response)
        .context("Failed to extract JSON from agent response")?;

    let prd: Prd =
        serde_json::from_str(&cleaned).context("Failed to parse generated PRD JSON")?;

    validate_prd(&prd).context("PRD validation failed")?;

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

    save_prd(&output_path, &prd).context("Failed to save PRD")?;
    eprintln!(
        "[ramph] PRD saved successfully to {}!",
        output_path.display()
    );

    Ok(())
}
