use anyhow::{Context, Result};
use chrono::Local;
use colored::Colorize;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::amp::run_iteration;
use crate::output;
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

    let initial_prd = load_prd(&prd_path)?;
    let total_stories = initial_prd.stories.len() as u64;
    let completed_initial = initial_prd.stories.iter().filter(|s| s.passes).count() as u64;

    output::header(&format!("=== ramph run ==="));
    output::info(&format!(
        "PRD: {} ({} stories, {} completed)",
        prd_path.display(),
        total_stories,
        completed_initial
    ));

    let progress_bar = output::create_progress_bar(total_stories);
    progress_bar.set_position(completed_initial);

    for iteration in 1..=max_iterations {
        let prd = load_prd(&prd_path)?;

        let Some(story) = prd.get_next_story() else {
            progress_bar.finish_and_clear();
            output::success("All stories complete!");
            break;
        };

        let story_id = story.id.clone();
        let story_title = story.title.clone();

        output::header(&format!(
            "=== Iteration {}/{} ===",
            iteration, max_iterations
        ));
        output::story_status(&story_id, &story_title, output::StoryStatus::Running);

        let progress = load_progress(&progress_path)?;
        let prompt = build_iteration_prompt(&base_prompt, story, &progress);

        let spinner = output::create_spinner(&format!("Working on {}...", story_id));

        match run_iteration(&prompt, &cwd, Some(&spinner)).await {
            Ok(_output) => {
                output::finish_spinner_success(
                    &spinner,
                    &format!("Completed: {} - {}", story_id, story_title),
                );

                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                append_progress(
                    &progress_path,
                    &format!("\n## [{timestamp}] Completed: {story_id}\n"),
                )?;

                let prd = load_prd(&prd_path)?;
                let completed = prd.stories.iter().filter(|s| s.passes).count() as u64;
                progress_bar.set_position(completed);
            }
            Err(e) => {
                output::finish_spinner_error(&spinner, &format!("Failed: {} - {}", story_id, e));

                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                append_progress(
                    &progress_path,
                    &format!("\n## [{timestamp}] Failed: {story_id}\nError: {e}\n"),
                )?;
            }
        }
    }

    progress_bar.finish_and_clear();
    print_final_summary(&prd_path)?;

    Ok(())
}

fn print_final_summary(prd_path: &PathBuf) -> Result<()> {
    let prd = load_prd(prd_path)?;
    let total = prd.stories.len();
    let completed = prd.stories.iter().filter(|s| s.passes).count();
    let remaining = total - completed;

    output::header("=== Summary ===");

    if remaining == 0 {
        eprintln!(
            "  {} All {} stories completed!",
            "✓".green().bold(),
            total
        );
    } else {
        eprintln!(
            "  {} {}/{} stories completed, {} remaining",
            "•".blue().bold(),
            completed,
            total,
            remaining
        );
    }

    Ok(())
}

pub async fn run_plan_command(
    cwd: PathBuf,
    output_file: PathBuf,
    description: Option<String>,
    force: bool,
) -> Result<()> {
    let output_path = cwd.join(&output_file);

    check_output_file(&output_path, force).context("Output file validation failed")?;

    output::header("=== ramph plan ===");
    output::info("Starting planning conversation...");
    output::info("The AI agent will help you break down your project into stories.\n");

    let prompt = build_planning_prompt(description);
    let spinner = output::create_spinner("Planning session in progress...");

    let conversation = run_iteration(&prompt, &cwd, Some(&spinner))
        .await
        .context("Planning session failed")?;

    output::finish_spinner_success(&spinner, "Planning conversation complete!");

    output::info("Generating structured PRD from conversation...\n");
    let extraction_spinner = output::create_spinner("Extracting PRD...");

    let extraction_prompt = build_extraction_prompt(&conversation);
    let json_response = run_iteration(&extraction_prompt, &cwd, Some(&extraction_spinner)).await?;

    output::finish_spinner_success(&extraction_spinner, "PRD extracted!");

    let cleaned = clean_json_response(&json_response)
        .context("Failed to extract JSON from agent response")?;

    let prd: Prd =
        serde_json::from_str(&cleaned).context("Failed to parse generated PRD JSON")?;

    validate_prd(&prd).context("PRD validation failed")?;

    display_prd_summary(&prd);

    eprint!(
        "\n{} Save this PRD to {}? (y/n): ",
        "?".cyan().bold(),
        output_path.display()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        output::warn("PRD not saved. Exiting.");
        return Ok(());
    }

    save_prd(&output_path, &prd).context("Failed to save PRD")?;
    output::success(&format!("PRD saved to {}!", output_path.display()));

    Ok(())
}
