use reqwest;
use serde_json::json;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the git diff
    let diff = get_git_diff()?;
    println!("diff {}", diff);

    // Get commit messages
    let commit_messages = get_commit_messages()?;

    // Get linked issue (if any)
    let issue = get_linked_issue()?;

    // Generate PR description using AI
    let pr_description = generate_pr_description(&diff, &commit_messages, issue.unwrap())?;

    // Create pull request
    create_pull_request(&pr_description)?;

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

fn generate_pr_description(
    diff: &str,
    commit_messages: &[String],
    issue: String,
) -> Result<String, Box<dyn std::error::Error>> {
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

    // let args = CompletionArgs::new("text-davinci-002", prompt).max_tokens(500);
    // let result = client.complete(args)?;
    Ok(prompt)

    // Ok(result.choices[0].text.clone())
}

fn create_pull_request(description: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let github_token = std::env::var("GITHUB_TOKEN")?;
    let repo_owner = "your_github_username";
    let repo_name = "your_repo_name";

    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls",
        repo_owner, repo_name
    );

    let body = json!({
        "title": "Automated Pull Request",
        "body": description,
        "head": "your_branch_name",
        "base": "master"
    });

    let _response = client
        .post(&url)
        .header("Authorization", format!("token {}", github_token))
        .header("User-Agent", "pullrequest-app")
        .json(&body)
        .send();

    // if responsestatus().is_success() {
    //     println!("Pull request created successfully!");
    // } else {
    //     println!(
    //         "Failed to create pull request. Status: {}",
    //         response.status()
    //     );
    // }

    Ok(())
}
