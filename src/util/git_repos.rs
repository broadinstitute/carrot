//! Provides functions for doing operations related to git repos

use crate::config::PrivateGithubAccessConfig;
use std::process::Command;

/// Struct for checking the existence/accessibility of git repos
#[derive(Clone)]
pub struct GitRepoChecker {
    private_github_config: Option<PrivateGithubAccessConfig>,
}

impl GitRepoChecker {
    /// Creates a new GitRepoChecker that will use private_github_config to access private github
    /// repos, if supplied
    pub fn new(private_github_config: Option<PrivateGithubAccessConfig>) -> GitRepoChecker {
        GitRepoChecker {
            private_github_config,
        }
    }

    /// Checks where the remote git repo specified by `url` exists
    ///
    /// Uses the `git ls-remote` command to check the specified url for a git repo.  Returns Ok(true)
    /// if the command is successful, and Ok(false) if it fails.  Returns an error if there is some
    /// error trying to execute the command
    pub fn git_repo_exists(&self, url: &str) -> Result<bool, std::io::Error> {
        // Get the url with credentials if it's a github url and we have a configuration for private
        // github access
        let url_to_check = if url.contains("github.com") {
            if let Some(private_github_config) = &self.private_github_config {
                GitRepoChecker::format_github_url_with_creds(
                    url,
                    private_github_config.client_id(),
                    private_github_config.client_token(),
                )
            } else {
                url.to_string()
            }
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
}

#[cfg(test)]
mod tests {

    use super::*;

    #[actix_rt::test]
    async fn git_repo_exists_true() {
        let git_repo_checker = GitRepoChecker::new(None);
        let test = git_repo_checker
            .git_repo_exists("https://github.com/broadinstitute/gatk.git")
            .expect("Error when checking if git repo exists");

        assert!(test);
    }
    #[actix_rt::test]
    async fn git_repo_exists_false() {
        let git_repo_checker = GitRepoChecker::new(None);
        let test = git_repo_checker
            .git_repo_exists("https://example.com/example/project.git")
            .expect("Error when checking if git repo exists");

        assert!(!test);
    }

    #[test]
    fn format_github_url_with_creds_with_www() {
        let test = GitRepoChecker::format_github_url_with_creds(
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
        let test = GitRepoChecker::format_github_url_with_creds(
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
        let test = GitRepoChecker::format_github_url_with_creds(
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
