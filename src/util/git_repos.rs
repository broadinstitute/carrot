//! Provides functions for doing operations related to git repos

use crate::config;
use std::process::Command;

/// Checks where the remote git repo specified by `url` exists
///
/// Uses the `git ls-remote` command to check the specified url for a git repo.  Returns Ok(true)
/// if the command is successful, and Ok(false) if it fails.  Returns an error if there is some
/// error trying to execute the command
pub async fn git_repo_exists(url: &str) -> Result<bool, std::io::Error> {
    let url_to_check = if *config::ENABLE_PRIVATE_GITHUB_ACCESS && url.contains("github.com") {
        format_github_url_with_creds(
            url,
            &*config::PRIVATE_GITHUB_CLIENT_ID.as_ref().unwrap(),
            &*config::PRIVATE_GITHUB_CLIENT_TOKEN.as_ref().unwrap(),
        )
    } else {
        url.to_string()
    };

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("git ls-remote {}", url_to_check))
        .output()?;

    if output.status.success() {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Takes a github url, username, and password and returns the url to use for cloning with those
/// credentials, in the form https://username:password@github.com/some/repo.git
fn format_github_url_with_creds(url: &str, username: &str, password: &str) -> String {
    // Trim https://www. from start of url so we can stick the credentials in there
    let trimmed_url = url
        .trim_start_matches("https://")
        .trim_start_matches("www.");
    // Format url with auth creds and return
    format!("https://{}:{}@{}", username, password, trimmed_url)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[actix_rt::test]
    async fn git_repo_exists_true() {
        let test = git_repo_exists("https://github.com/broadinstitute/gatk.git")
            .await
            .expect("Error when checking if git repo exists");

        assert!(test);
    }
    #[actix_rt::test]
    async fn git_repo_exists_false() {
        let test = git_repo_exists("https://example.com/example/project.git")
            .await
            .expect("Error when checking if git repo exists");

        assert!(!test);
    }

    #[test]
    fn format_github_url_with_creds_with_www() {
        let test = format_github_url_with_creds(
            "https://www.example.com/example/project.git",
            "test_user",
            "test_pass",
        );

        assert_eq!(
            test,
            "https://test_user:test_pass@example.com/example/project.git"
        );
    }

    #[test]
    fn format_github_url_with_creds_without_www() {
        let test = format_github_url_with_creds(
            "https://example.com/example/project.git",
            "test_user",
            "test_pass",
        );

        assert_eq!(
            test,
            "https://test_user:test_pass@example.com/example/project.git"
        );
    }

    #[test]
    fn format_github_url_with_creds_without_https() {
        let test = format_github_url_with_creds(
            "example.com/example/project.git",
            "test_user",
            "test_pass",
        );

        assert_eq!(
            test,
            "https://test_user:test_pass@example.com/example/project.git"
        );
    }
}
