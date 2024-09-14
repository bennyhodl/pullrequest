use anthropic::{client::ClientBuilder, types::CompleteRequestBuilder, AI_PROMPT, HUMAN_PROMPT};
use anyhow::anyhow;
use colored::*;
use dotenv::dotenv;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::future::Future;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;

fn check_uncommitted_changes() -> Result<(), anyhow::Error> {
    let output = Command::new("git")
        .args(&["status", "--porcelain"])
        .output()?;

    if !output.stdout.is_empty() {
        eprintln!(
            "{}",
            "There are uncommitted changes. Please commit or stash them before proceeding."
                .bright_red()
        );
        std::process::exit(1);
    }

    Ok(())
}

fn push_to_remote(current_branch: &str) -> Result<(), anyhow::Error> {
    let status = Command::new("git")
        .args(&["push", "origin", current_branch])
        .spawn()
        .expect("Could not push");

    // if !status.success() {
    //     eprintln!(
    //         "{}",
    //         "Failed to push to remote. Please ensure your branch is up to date with origin."
    //             .bright_red()
    //     );
    //     std::process::exit(1);
    // }

    Ok(())
}

fn check_for_remote() -> Result<(), anyhow::Error> {
    // Get the current branch name
    let current_branch = get_current_branch()?;

    // Check if the branch has a remote
    if !has_remote(&current_branch)? {
        // If no remote, push to origin
        push_to_remote(&current_branch)?;
    }

    Ok(())
}

fn get_current_branch() -> Result<String, anyhow::Error> {
    let output = Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        Err(anyhow!("Failed to get current branch"))
    }
}

fn has_remote(branch: &str) -> Result<bool, anyhow::Error> {
    let output = Command::new("git")
        .args(&["ls-remote", "--exit-code", "--heads", "origin", branch])
        .output()?;

    Ok(output.status.success())
}

fn create_progress_bar(multi_progress: &MultiProgress, message: &str) -> ProgressBar {
    let pb = multi_progress.add(ProgressBar::new(1));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(message.to_string());
    pb
}

async fn run_with_progress_async<F, T>(pb: Arc<ProgressBar>, f: F) -> Result<T, anyhow::Error>
where
    F: Future<Output = Result<T, anyhow::Error>> + Send + 'static,
    T: Send + 'static,
{
    let result = f.await;
    match &result {
        Ok(_) => pb.finish_with_message(format!("{} Done", pb.message()).green().to_string()),
        Err(_) => pb.finish_with_message(format!("{} Failed", pb.message()).red().to_string()),
    }
    result
}

async fn run_with_progress<F, T>(pb: Arc<ProgressBar>, f: F) -> Result<T, anyhow::Error>
where
    F: FnOnce() -> Result<T, anyhow::Error> + Send + 'static,
    T: Send + 'static,
{
    let result = tokio::task::spawn_blocking(f).await?;
    match &result {
        Ok(_) => pb.finish_with_message(format!("{} Done", pb.message()).green().to_string()),
        Err(_) => pb.finish_with_message(format!("{} Failed", pb.message()).red().to_string()),
    }
    result
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();
    // let github_token = std::env::var("GITHUB_TOKEN").expect("no gh key");
    let anthropic_key = std::env::var("ANTHROPIC_KEY").expect("no anthropic key");

    check_uncommitted_changes()?;

    println!("{}", "Starting pullrequest process...".blue().bold());

    let multi_progress = Arc::new(MultiProgress::new());
    let mp = Arc::clone(&multi_progress);

    let remote_pb = Arc::new(create_progress_bar(&mp, "Checking remote"));
    let diff_pb = Arc::new(create_progress_bar(&mp, "Getting git diff"));
    let commits_pb = Arc::new(create_progress_bar(&mp, "Getting commit messages"));
    let issue_pb = Arc::new(create_progress_bar(&mp, "Checking linked issue"));
    let description_pb = Arc::new(create_progress_bar(&mp, "Generating PR description"));
    let pr_pb = Arc::new(create_progress_bar(&mp, "Creating pull request"));

    run_with_progress(remote_pb.clone(), || check_for_remote()).await?;

    let diff = run_with_progress(diff_pb.clone(), || get_git_diff()).await?;
    let commit_messages = run_with_progress(commits_pb.clone(), || get_commit_messages()).await?;
    let issue = run_with_progress(issue_pb.clone(), || get_linked_issue()).await?;

    let anthropic_key_clone = anthropic_key.clone();
    let pr_description = run_with_progress_async(description_pb.clone(), async move {
        generate_pr_description(&diff, &commit_messages, issue, anthropic_key_clone).await
    })
    .await?;

    run_with_progress(pr_pb.clone(), move || create_pull_request(&pr_description)).await?;

    multi_progress.clear()?;

    println!("{}", "pullrequest process completed.".green().bold());

    Ok(())
}

fn get_git_diff() -> Result<String, anyhow::Error> {
    let output = Command::new("git")
        .args(&["diff", "origin/master"])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn get_commit_messages() -> Result<Vec<String>, anyhow::Error> {
    let output = Command::new("git")
        .args(&["log", "origin/master..HEAD", "--pretty=format:%s"])
        .output()?;

    let messages = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(String::from)
        .collect();

    Ok(messages)
}

fn get_linked_issue() -> Result<Option<String>, anyhow::Error> {
    // This function would need to be implemented to fetch the linked issue from GitHub
    // It might involve parsing commit messages or branch names for issue numbers
    // and then querying the GitHub API
    Ok(None)
}

async fn generate_pr_description(
    diff: &str,
    commit_messages: &[String],
    issue: Option<String>,
    anthropic_key: String,
) -> Result<String, anyhow::Error> {
    dotenv().ok();
    // let client = ApiClient::new()?;
    let prompt = format!(
        "Generate a pull request description based on the following information:\n\
         Diff: {}\n\
         Commit messages: {}\n\
         Linked issue: {:?}\n\
         Please summarize the changes, their purpose, and any potential impact.",
        diff,
        commit_messages.join("\n"),
        issue
    );

    let claude = ClientBuilder::default()
        .api_key(anthropic_key)
        .default_model("claude-3-haiku-20240307".to_string())
        .build()?;

    let request = CompleteRequestBuilder::default()
        .prompt(format!("{HUMAN_PROMPT}{}\n{AI_PROMPT}", prompt))
        .stream(false)
        .max_tokens_to_sample(1_000_000 as usize)
        .stop_sequences(vec![HUMAN_PROMPT.to_string()])
        .build()?;
    let chat = claude.complete(request).await?;
    Ok(chat.completion)
}

fn create_pull_request(description: &str) -> Result<(), anyhow::Error> {
    Command::new("gh")
        .args(&[
            "pr",
            "create",
            "--title",
            "Automated Pull Request",
            "--body",
            description,
            "--base",
            "master",
        ])
        .exec();

    Ok(())
}
