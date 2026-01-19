use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    #[default]
    Normal,
    Verbose,
    Quiet,
}

static OUTPUT_MODE: OnceLock<OutputMode> = OnceLock::new();

pub fn init(mode: OutputMode, no_color: bool) {
    OUTPUT_MODE.set(mode).ok();
    if no_color {
        colored::control::set_override(false);
    }
}

pub fn mode() -> OutputMode {
    *OUTPUT_MODE.get().unwrap_or(&OutputMode::Normal)
}

pub fn is_quiet() -> bool {
    mode() == OutputMode::Quiet
}

pub fn is_verbose() -> bool {
    mode() == OutputMode::Verbose
}

pub fn success(msg: &str) {
    if !is_quiet() {
        eprintln!("{} {}", "✓".green().bold(), msg);
    }
}

pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

pub fn warn(msg: &str) {
    if !is_quiet() {
        eprintln!("{} {}", "⚠".yellow().bold(), msg);
    }
}

pub fn info(msg: &str) {
    if !is_quiet() {
        eprintln!("{} {}", "•".blue().bold(), msg);
    }
}

pub fn verbose(msg: &str) {
    if is_verbose() {
        eprintln!("{} {}", "›".dimmed(), msg.dimmed());
    }
}

pub fn header(msg: &str) {
    if !is_quiet() {
        eprintln!("\n{}", msg.bold());
    }
}

pub fn story_status(id: &str, title: &str, status: StoryStatus) {
    if is_quiet() {
        return;
    }
    let (icon, style) = match status {
        StoryStatus::Pending => ("○".dimmed(), title.dimmed()),
        StoryStatus::Running => ("⚙".yellow().bold(), title.yellow()),
        StoryStatus::Success => ("✓".green().bold(), title.green()),
        StoryStatus::Failed => ("✗".red().bold(), title.red()),
    };
    eprintln!("  {} {} {}", icon, id.bold(), style);
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum StoryStatus {
    Pending,
    Running,
    Success,
    Failed,
}

pub fn create_spinner(msg: &str) -> ProgressBar {
    if is_quiet() {
        return ProgressBar::hidden();
    }
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(msg.to_string());
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

pub fn create_progress_bar(total: u64) -> ProgressBar {
    if is_quiet() {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold} [{bar:20.cyan/dim}] {pos}/{len} stories")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb.set_prefix("[ramph]");
    pb
}

#[allow(dead_code)]
pub fn create_multi_progress() -> MultiProgress {
    if is_quiet() {
        MultiProgress::with_draw_target(indicatif::ProgressDrawTarget::hidden())
    } else {
        MultiProgress::new()
    }
}

pub fn finish_spinner_success(spinner: &ProgressBar, msg: &str) {
    spinner.set_style(ProgressStyle::default_spinner().template("{msg}").unwrap());
    spinner.finish_with_message(format!("{} {}", "✓".green().bold(), msg));
}

pub fn finish_spinner_error(spinner: &ProgressBar, msg: &str) {
    spinner.set_style(ProgressStyle::default_spinner().template("{msg}").unwrap());
    spinner.finish_with_message(format!("{} {}", "✗".red().bold(), msg));
}
