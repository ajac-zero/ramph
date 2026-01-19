# ramph

An autonomous coding agent that works through user stories in your project.

## What it does

ramph takes a list of tasks (user stories) and executes them one by one using AI. It implements features, runs tests, commits changes, and moves on to the next story—all without manual intervention.

## Getting Started

### 1. Install

```bash
cargo install ramph
```

### 2. Create a PRD

You can either create a `prd.json` manually or let ramph help:

```bash
ramph plan
```

This starts an interactive session to define your stories.

### 3. Run

```bash
ramph run
```

ramph will work through each story, verify changes pass tests, and commit.

## PRD Format

Create a `prd.json` in your project root:

```json
{
  "branchName": "feature/my-feature",
  "stories": [
    {
      "id": "STORY-001",
      "title": "Add user authentication",
      "description": "Implement JWT-based authentication for the API",
      "priority": 1,
      "passes": false,
      "acceptance_criteria": [
        "Login endpoint returns JWT token",
        "Protected routes require valid token"
      ]
    }
  ]
}
```

Stories are executed in priority order. Lower numbers run first.

## Commands

| Command | Description |
|---------|-------------|
| `ramph run` | Execute stories from a PRD |
| `ramph plan` | Interactively create a new PRD |

## Options

```
--verbose, -v    Show detailed output including tool calls
--quiet, -q      Minimal output for CI environments
--no-color       Disable colored output
```

## How it works

For each incomplete story, ramph:

1. Reads the task requirements
2. Implements the changes
3. Runs tests to verify
4. Commits if tests pass
5. Marks the story complete
6. Moves to the next story
