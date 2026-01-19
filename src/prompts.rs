use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::types::Story;

// Embedded default resources
pub const DEFAULT_PROMPT: &str = include_str!("../prompt.md");
pub const DEFAULT_PROGRESS_TEMPLATE: &str = include_str!("../progress.txt");

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

pub fn load_prompt(path: Option<&PathBuf>) -> Result<String> {
    match path {
        Some(p) => fs::read_to_string(p)
            .with_context(|| format!("Failed to read custom prompt file: {}", p.display())),
        None => Ok(DEFAULT_PROMPT.to_string()),
    }
}

pub fn load_progress(path: &PathBuf) -> Result<String> {
    if path.exists() {
        fs::read_to_string(path)
            .with_context(|| format!("Failed to read progress file: {}", path.display()))
    } else {
        Ok(DEFAULT_PROGRESS_TEMPLATE.to_string())
    }
}

pub fn append_progress(path: &PathBuf, entry: &str) -> Result<()> {
    let mut content = load_progress(path).unwrap_or_default();
    content.push_str(entry);
    content.push('\n');
    fs::write(path, content).context("Failed to write progress.txt")
}

pub fn build_iteration_prompt(base_prompt: &str, story: &Story, progress: &str) -> String {
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

pub fn build_planning_prompt(initial_description: Option<String>) -> String {
    let initial_context = match initial_description {
        Some(desc) => format!(
            "\n## Initial Project Description\n\n{}\n\nStart by clarifying any questions about this description.",
            desc
        ),
        None => String::new(),
    };

    PLANNING_PROMPT_TEMPLATE.replace("{initial_context}", &initial_context)
}

pub fn build_extraction_prompt(conversation_history: &str) -> String {
    EXTRACTION_PROMPT.replace("{conversation_history}", conversation_history)
}

pub fn clean_json_response(response: &str) -> Result<String> {
    let trimmed = response.trim();

    // Remove markdown code fences if present
    let without_fences = if trimmed.starts_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() > 2 {
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
