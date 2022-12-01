//! Provides functions for doing operations related to git repos

use crate::config::PrivateGithubAccessConfig;
use chrono::NaiveDateTime;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fmt, fs};
use uuid::Uuid;

/// Struct for checking the existence/accessibility of git repos
#[derive(Clone)]
pub struct GitRepoManager {
    private_github_config: Option<PrivateGithubAccessConfig>,
    repo_cache_location: String,
}

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Git(String),
    NotFound(String),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(e) => write!(f, "GitRepo Error IO {}", e),
            Error::Git(e) => write!(f, "GitRepo Error Git {}", e),
            Error::NotFound(e) => write!(f, "GitRepo Error NotFound commit or tag {}", e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

impl GitRepoManager {
    /// Creates a new GitRepoChecker that will use private_github_config to access private github
    /// repos, if supplied
    pub fn new(
        private_github_config: Option<PrivateGithubAccessConfig>,
        repo_cache_location: String,
    ) -> GitRepoManager {
        GitRepoManager {
            private_github_config,
            repo_cache_location,
        }
    }

    /// Attempts to download the git repo at `url` into the git repo cache at
    /// `&self.repo_cache_location` in a new subdirectory `subdir`.  Returns an error if it fails
    /// for any reason
    pub fn download_git_repo(&self, software_id: Uuid, url: &str) -> Result<(), Error> {
        // Get the url with credentials if it's a github url and we have a configuration for private
        // github access
        let url_to_check = self.process_url_for_github_creds(url);
        // Get the directory path we'll write to
        let directory: PathBuf = [&self.repo_cache_location, &software_id.to_string()]
            .iter()
            .collect();
        // Create the repo directory and subdir if they don't already exist
        fs::create_dir_all(&directory)?;

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("git clone -n {} {}", url_to_check, directory.to_str().expect("Failed to convert directory for git repo into string.  This should not happen.")))
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Git(
                String::from_utf8_lossy(&*output.stderr).to_string(),
            ))
        }
    }

    /// Runs git fetch on the cached repo for the software specified by `software_id`, determines
    /// whether `commit_or_tag` is a commit or tag, and if it is a commit, returns the commit and
    /// the tags if the commit has any, and if it is a tag, returns the commit for that tag and any
    /// tags for that commit.  If it's neither, returns Error::NotFound.  Returns an error for any
    /// other failures
    pub fn get_commit_and_tags_and_date_from_commit_or_tag(
        &self,
        software_id: Uuid,
        commit_or_tag: &str,
    ) -> Result<(String, Vec<String>, NaiveDateTime), Error> {
        let subdir: String = software_id.to_string();
        // Get the directory path for the git repo
        let repo_dir: PathBuf = [&self.repo_cache_location, &subdir].iter().collect();
        // Run git fetch on the repo so it's up to date
        self.git_fetch(&repo_dir)?;
        // Check first if it's a tag
        let (commit, tags): (String, Vec<String>) =
            if self.git_show_ref_verify(&repo_dir, commit_or_tag)? {
                // Get the commit for this tag
                let commit = self.git_rev_list(&repo_dir, commit_or_tag)?;
                // Get all tags for this commit (in case there are multiple)
                let tags = self.git_tag_points_at(&repo_dir, &commit)?;
                (commit, tags)
            }
            // If it's not, we'll check if it's a commit
            else if self.git_rev_parse_verify(&repo_dir, commit_or_tag)? {
                // Check if we have a tag for this commit
                let tags = self.git_tag_points_at(&repo_dir, commit_or_tag)?;
                (commit_or_tag.to_owned(), tags)
            }
            // If it's neither, we'll return an error
            else {
                return Err(Error::NotFound(commit_or_tag.to_string()));
            };

        // Get the timestamp for the commit
        let timestamp: NaiveDateTime = self.git_show_date(&repo_dir, &commit)?;

        Ok((commit, tags, timestamp))
    }

    /// Runs git fetch on the git repo in `repo_dir`.  Returns an error if that fails
    fn git_fetch(&self, repo_dir: &Path) -> Result<(), Error> {
        // Run the command
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!("git fetch --tags -p -P"))
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Git(
                String::from_utf8_lossy(&*output.stderr).to_string(),
            ))
        }
    }

    /// Runs git rev-parse on the git repo in `repo_dir` to see if `commit_maybe` is a commit in the
    /// repo.  Returns true if it is an false if it's not.  If it fails for any other reason,
    /// returns an error.  It should be noted that a true returned from this function does not
    /// mean `commit_maybe` is not a tag.  This will also return true for tags.  If using this to
    /// differentiate between tags and commits, `git_show_ref_verify` should be run first to check
    /// if the string in question is a tag
    fn git_rev_parse_verify(&self, repo_dir: &Path, commit_maybe: &str) -> Result<bool, Error> {
        // Run the command
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!(
                "git rev-parse --verify \"{}^{{commit}}\"",
                commit_maybe
            ))
            .output()?;

        // If the command was successful, return true
        if output.status.success() {
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&*output.stderr).to_string();
            // If stderr matches the message that git spits out if the commit is not valid, return
            // false
            if stderr.trim() == "fatal: Needed a single revision" {
                return Ok(false);
            }
            // Otherwise return an error
            Err(Error::Git(stderr))
        }
    }

    /// Uses git show-ref to verify whether `tag_maybe` is a tag.  Returns true if it is, or false
    /// if it's not.  Returns an error if the command fails for some reason.
    fn git_show_ref_verify(&self, repo_dir: &Path, tag_maybe: &str) -> Result<bool, Error> {
        // Run the command
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!("git show-ref --verify \"refs/tags/{}\"", tag_maybe))
            .output()?;

        // If the command was successful, return true
        if output.status.success() {
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&*output.stderr).to_string();
            // If stderr says it's not a valid ref, we'll just return false
            if stderr.contains("- not a valid ref") {
                return Ok(false);
            }
            // Otherwise return an error
            Err(Error::Git(stderr))
        }
    }

    /// Calls git rev-list on the repo in `repo_dir` for `tag` and returns the resulting commit if
    /// successful, or an error if the command fails for some other reason
    fn git_rev_list(&self, repo_dir: &Path, tag: &str) -> Result<String, Error> {
        // Run the command
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!("git rev-list -n 1 {}", tag))
            .output()?;

        // If the command was successful, return the commit string
        if output.status.success() {
            Ok(String::from_utf8_lossy(&*output.stdout)
                .to_string()
                .trim()
                .to_string())
        } else {
            // Otherwise return an error
            Err(Error::Git(
                String::from_utf8_lossy(&*output.stderr).to_string(),
            ))
        }
    }

    /// Calls git tag --points-at on the repo in `repo_dir` for `commit` and returns the resulting
    /// tags if successful, or an error if the command fails for some other reason
    fn git_tag_points_at(&self, repo_dir: &Path, commit: &str) -> Result<Vec<String>, Error> {
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!("git tag --points-at {}", commit))
            .output()?;

        if output.status.success() {
            let stdout: String = String::from_utf8_lossy(&*output.stdout).to_string();
            // Split the output on newlines to get the list of tags
            Ok(stdout.split_terminator("\n").map(String::from).collect())
        } else {
            Err(Error::Git(
                String::from_utf8_lossy(&*output.stderr).to_string(),
            ))
        }
    }

    /// Uses git show to get the commit date of `commit` and returns it as a NaiveDateTime if
    /// found, otherwise returns an error
    fn git_show_date(&self, repo_dir: &Path, commit: &str) -> Result<NaiveDateTime, Error> {
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!(
                "git show --no-patch --no-notes --pretty='%cd' {}",
                commit
            ))
            .output()?;

        if output.status.success() {
            let stdout: String = String::from_utf8_lossy(&*output.stdout).trim().to_string();
            // Attempt to parse the output as a datetime
            Ok(
                NaiveDateTime::parse_from_str(&stdout, "%a %b %-d %T %Y %z").expect(&format!(
                    "Failed to parse timestamp {} for commit {}. This should not happen.",
                    &stdout, commit
                )),
            )
        } else {
            Err(Error::Git(
                String::from_utf8_lossy(&*output.stderr).to_string(),
            ))
        }
    }

    /// Checks if `url` is a github url.  If it is, and `&self` has credentials for connecting to
    /// private github repos, adds those creds to the url and returns.  Otherwise, returns the url
    /// unchanged as a String
    fn process_url_for_github_creds(&self, url: &str) -> String {
        if url.contains("github.com") {
            if let Some(private_github_config) = &self.private_github_config {
                GitRepoManager::format_github_url_with_creds(
                    url,
                    private_github_config.client_id(),
                    private_github_config.client_token(),
                )
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
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
    use std::env;
    use std::fs::read_to_string;
    use std::str::FromStr;
    use tempfile::TempDir;

    fn create_test_git_repo() -> (TempDir, String, String, NaiveDateTime, NaiveDateTime) {
        // Create a tempdir we'll put the repo in
        let temp_repo_dir = TempDir::new().unwrap();
        // Make a dir for the repo
        let mut repo_dir_path: PathBuf = temp_repo_dir.path().to_path_buf();
        repo_dir_path.push(PathBuf::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap());
        fs::create_dir(&repo_dir_path).unwrap();
        // Load script we'll run
        let script = read_to_string("testdata/util/git_repo/create_test_repo.sh").unwrap();
        // Run script for filling repo
        let output = Command::new("sh")
            .current_dir(repo_dir_path)
            .arg("-c")
            .arg(script)
            .output()
            .unwrap();

        let mut commits: Vec<String> = String::from_utf8_lossy(&*output.stdout)
            .split_terminator("\n")
            .map(String::from)
            .collect();

        let second_commit_date =
            NaiveDateTime::parse_from_str(&commits.pop().unwrap(), "%a %b %-d %T %Y %z").unwrap();
        let first_commit_date =
            NaiveDateTime::parse_from_str(&commits.pop().unwrap(), "%a %b %-d %T %Y %z").unwrap();
        let second_commit = commits.pop().unwrap();
        let first_commit = commits.pop().unwrap();

        (
            temp_repo_dir,
            first_commit,
            second_commit,
            first_commit_date,
            second_commit_date,
        )
    }

    #[actix_rt::test]
    async fn get_commit_and_tags_from_commit_or_tag_success_commit() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            create_test_git_repo();
        let git_repo_manager =
            GitRepoManager::new(None, git_repo_temp_dir.path().to_str().unwrap().to_string());
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_commit_or_tag(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                &first_commit,
            )
            .expect("Error when checking for commit and tags");

        assert_eq!(commit, first_commit);
        assert_eq!(commit_date, first_date);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"first".to_string()));
        assert!(tags.contains(&"beginning".to_string()));
    }

    #[actix_rt::test]
    async fn get_commit_and_tags_from_commit_or_tag_success_commit_no_tags() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            create_test_git_repo();
        let git_repo_manager =
            GitRepoManager::new(None, git_repo_temp_dir.path().to_str().unwrap().to_string());
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_commit_or_tag(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                &second_commit,
            )
            .expect("Error when checking for commit and tags");

        assert_eq!(commit, second_commit);
        assert_eq!(commit_date, second_date);
        assert_eq!(tags.len(), 0);
    }

    #[actix_rt::test]
    async fn get_commit_and_tags_from_commit_or_tag_success_tag() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            create_test_git_repo();
        let git_repo_manager =
            GitRepoManager::new(None, git_repo_temp_dir.path().to_str().unwrap().to_string());
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_commit_or_tag(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                "first",
            )
            .expect("Error when checking for commit and tags");

        assert_eq!(commit, first_commit);
        assert_eq!(commit_date, first_date);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"first".to_string()));
        assert!(tags.contains(&"beginning".to_string()));
    }

    #[actix_rt::test]
    async fn get_commit_and_tags_from_commit_or_tag_failure_not_found() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            create_test_git_repo();
        let git_repo_manager =
            GitRepoManager::new(None, git_repo_temp_dir.path().to_str().unwrap().to_string());
        let commit_or_tag = String::from("last");
        let error = git_repo_manager
            .get_commit_and_tags_and_date_from_commit_or_tag(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                &commit_or_tag,
            )
            .expect_err("No error when checking for commit and tags");

        assert!(matches!(error, Error::NotFound(commit_or_tag)));
    }

    #[actix_rt::test]
    async fn get_commit_and_tags_from_commit_or_tag_failure_no_software() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            create_test_git_repo();
        let git_repo_manager =
            GitRepoManager::new(None, git_repo_temp_dir.path().to_str().unwrap().to_string());
        let error = git_repo_manager
            .get_commit_and_tags_and_date_from_commit_or_tag(
                Uuid::from_str("2d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                "first",
            )
            .expect_err("No error when checking for commit and tags");

        assert!(matches!(error, Error::IO(_)));
    }

    #[test]
    fn format_github_url_with_creds_with_www() {
        let test = GitRepoManager::format_github_url_with_creds(
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
        let test = GitRepoManager::format_github_url_with_creds(
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
        let test = GitRepoManager::format_github_url_with_creds(
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
