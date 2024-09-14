use anthropic::{client::ClientBuilder, types::CompleteRequestBuilder, AI_PROMPT, HUMAN_PROMPT};
use dotenv::dotenv;
use std::os::unix::process::CommandExt;
use std::process::Command;

fn check_uncommitted_changes() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(&["status", "--porcelain"])
        .output()?;

    if !output.stdout.is_empty() {
        eprintln!("There are uncommitted changes. Please commit or stash them before proceeding.");
        std::process::exit(1);
    }

    Ok(())
}

fn push_to_remote(current_branch: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("git")
        .args(&["push", "origin", current_branch])
        .status()?;

    if !status.success() {
        eprintln!("Failed to push to remote. Please ensure your branch is up to date with origin.");
        std::process::exit(1);
    }

    Ok(())
}

fn check_for_remote() -> Result<(), Box<dyn std::error::Error>> {
    // Get the current branch name
    let current_branch = get_current_branch()?;

    // Check if the branch has a remote
    if !has_remote(&current_branch)? {
        // If no remote, push to origin
        push_to_remote(&current_branch)?;
    }

    Ok(())
}

fn get_current_branch() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        Err("Failed to get current branch".into())
    }
}

fn has_remote(branch: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(&["ls-remote", "--exit-code", "--heads", "origin", branch])
        .output()?;

    Ok(output.status.success())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let github_token = std::env::var("GITHUB_TOKEN").expect("no gh key");
    let anthropic_key = std::env::var("ANTHROPIC_KEY").expect("no anthropic key");

    // Check for uncommitted changes
    check_uncommitted_changes()?;

    check_for_remote()?;

    // Push to remote
    push_to_remote(&get_current_branch()?)?;

    // Get the git diff
    let diff = get_git_diff()?;

    // Get commit messages
    let commit_messages = get_commit_messages()?;

    // Get linked issue (if any)
    let issue = get_linked_issue()?;

    // Generate PR description using AI
    println!("Generating AI description with diffs...");
    let pr_description =
        generate_pr_description(&diff, &commit_messages, issue, anthropic_key).await?;
    println!("Description: {}", pr_description);

    // Create pull request
    println!("Creating pull request...");
    create_pull_request(&pr_description, github_token).await?;

    Ok(())
}

fn get_git_diff() -> Result<String, std::io::Error> {
    let output = Command::new("git")
        .args(&["diff", "origin/master"])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn get_commit_messages() -> Result<Vec<String>, std::io::Error> {
    let output = Command::new("git")
        .args(&["log", "origin/master..HEAD", "--pretty=format:%s"])
        .output()?;

    let messages = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(String::from)
        .collect();

    Ok(messages)
}

fn get_linked_issue() -> Result<Option<String>, Box<dyn std::error::Error>> {
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
) -> Result<String, Box<dyn std::error::Error>> {
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

async fn create_pull_request(
    description: &str,
    _github_token: String,
) -> Result<(), Box<dyn std::error::Error>> {
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
