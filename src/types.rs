use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::output;

#[derive(Debug, Serialize, Deserialize)]
pub struct Prd {
    #[serde(rename = "branchName")]
    pub branch_name: String,
    pub stories: Vec<Story>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Story {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub passes: bool,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

impl Prd {
    pub fn get_next_story(&self) -> Option<&Story> {
        self.stories
            .iter()
            .filter(|s| !s.passes)
            .min_by_key(|s| s.priority)
    }
}

pub fn load_prd(path: &PathBuf) -> Result<Prd> {
    let content = fs::read_to_string(path).context("Failed to read prd.json")?;
    serde_json::from_str(&content).context("Failed to parse prd.json")
}

pub fn save_prd(path: &PathBuf, prd: &Prd) -> Result<()> {
    let content = serde_json::to_string_pretty(prd)?;
    fs::write(path, content).context("Failed to write prd.json")
}

pub fn validate_prd(prd: &Prd) -> Result<()> {
    anyhow::ensure!(!prd.branch_name.is_empty(), "Branch name cannot be empty");
    anyhow::ensure!(
        !prd.stories.is_empty(),
        "PRD must contain at least one story"
    );

    let mut seen_ids = HashSet::new();

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

pub fn display_prd_summary(prd: &Prd) {
    if output::is_quiet() {
        return;
    }

    eprintln!("\n{}", "=== PRD Summary ===".bold());
    eprintln!(
        "  {} {}",
        "Branch:".dimmed(),
        prd.branch_name.cyan()
    );
    eprintln!(
        "  {} {}\n",
        "Stories:".dimmed(),
        prd.stories.len().to_string().cyan()
    );

    for story in &prd.stories {
        let status_icon = if story.passes {
            "✓".green().bold()
        } else {
            "○".dimmed()
        };

        let priority_badge = format!("P{}", story.priority);
        let priority_colored = match story.priority {
            1 => priority_badge.red().bold(),
            2 => priority_badge.yellow(),
            3 => priority_badge.blue(),
            _ => priority_badge.dimmed(),
        };

        let title = if story.passes {
            story.title.green()
        } else {
            story.title.normal()
        };

        eprintln!(
            "  {} {} [{}] {}",
            status_icon,
            story.id.bold(),
            priority_colored,
            title
        );

        eprintln!(
            "      {} {} items",
            "Criteria:".dimmed(),
            story.acceptance_criteria.len()
        );
    }
    eprintln!();
}

pub fn check_output_file(path: &PathBuf, force: bool) -> Result<()> {
    if path.exists() && !force {
        anyhow::bail!(
            "PRD file already exists at {}. Use --force to overwrite.",
            path.display()
        );
    }
    Ok(())
}
