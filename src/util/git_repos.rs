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
    repo_cache_location: String,
    private_github_config: Option<PrivateGithubAccessConfig>,
}

/// To be implemented on structs of query params for finding tags/commits in a git repo
pub trait GitQuery {
    fn query(&self, git_repo_manager: &GitRepoManager, repo_dir: &Path) -> Result<Vec<String>, Error>;
}

/// Represents possible params for querying a git repo for a list of commits
#[derive(Debug)]
pub struct CommitQuery {
    pub branch: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub until: Option<NaiveDateTime>,
    pub number: Option<u32>,
}

impl GitQuery for CommitQuery {
    fn query(&self, git_repo_manager: &GitRepoManager, repo_dir: &Path) -> Result<Vec<String>, Error> {
        git_repo_manager.git_log_commits(repo_dir, self)
    }
}

/// Represents possible params for querying a git repo for a list of tags
#[derive(Debug)]
pub struct TagQuery {
    pub branch: Option<String>,
    pub number: usize
}

impl GitQuery for TagQuery {
    fn query(&self, git_repo_manager: &GitRepoManager, repo_dir: &Path) -> Result<Vec<String>, Error> {
        git_repo_manager.git_tags_list(repo_dir, self)
    }
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

        if let Some(config) = &private_github_config {
            Self::store_credentials(config)
                .expect("Encountered an error attempting to store github creds")
        }

        GitRepoManager {
            repo_cache_location,
            private_github_config
        }
    }

    /// Stores the specified github credentials so they will be used when cloning repos from github
    /// in case we need access to private ones
    fn store_credentials(
        config: &PrivateGithubAccessConfig
    ) -> Result<(), Error> {
        // Run the command
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("git config --global credential.helper store && echo \"https://{}:{}@github.com\" > ~/.git-credentials", config.client_id(), config.client_token()))
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Git(
                String::from_utf8_lossy(&*output.stderr).to_string().replace(config.client_id(), "*******").replace(config.client_token(), "*******"),
            ))
        }
    }

    /// Attempts to download the git repo at `url` into the git repo cache at
    /// `&self.repo_cache_location` in a new subdirectory `subdir`.  Returns an error if it fails
    /// for any reason
    pub fn download_git_repo(&self, software_id: Uuid, url: &str) -> Result<(), Error> {
        // Get the directory path we'll write to
        let directory: PathBuf = [&self.repo_cache_location, &software_id.to_string()]
            .iter()
            .collect();
        // Create the repo directory and subdir if they don't already exist
        fs::create_dir_all(&directory)?;

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("git clone -n {} {}", url, directory.to_str().expect("Failed to convert directory for git repo into string.  This should not happen.")))
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Git(
                self.censor_git_error(String::from_utf8_lossy(&*output.stderr).to_string()),
            ))
        }
    }

    /// Runs git fetch on the cached repo for the software specified by `software_id`, determines
    /// the commit hash for `git_ref`, and gets any tags for that commit along with the commit date.
    /// Returns the commit hash, tags, and commit date.  If `git_ref` is not a valid git ref, returns
    /// Error::NotFound.  Returns an error for any other failures
    pub fn get_commit_and_tags_and_date_from_ref(
        &self,
        software_id: Uuid,
        git_ref: &str,
    ) -> Result<(String, Vec<String>, NaiveDateTime), Error> {
        let subdir: String = software_id.to_string();
        // Get the directory path for the git repo
        let repo_dir: PathBuf = [&self.repo_cache_location, &subdir].iter().collect();
        // Run git fetch on the repo so it's up to date
        self.git_fetch(&repo_dir)?;
        // Get the commit hash for git_ref
        let commit: String = self.git_rev_parse_verify(&repo_dir, git_ref)?;
        // Get tags for this commit (if any)
        let tags: Vec<String> = self.git_tag_points_at(&repo_dir, &commit)?;
        // Get the timestamp for the commit
        let timestamp: NaiveDateTime = self.git_show_date(&repo_dir, &commit)?;

        Ok((commit, tags, timestamp))
    }

    /// Runs git fetch on the cached repo for the software specified by `software_id` and attempts
    /// to retrieve a list of commit hashes or tags for `query`
    pub fn query_repo_for_commits_or_tags(
        &self, software_id: Uuid, query: &impl GitQuery
    ) -> Result<Vec<String>, Error> {
        let subdir: String = software_id.to_string();
        // Get the directory path for the git repo
        let repo_dir: PathBuf = [&self.repo_cache_location, &subdir].iter().collect();
        // Run git fetch on the repo so it's up to date
        self.git_fetch(&repo_dir)?;
        // Retrieve commits/tags for query
        query.query(&self, &repo_dir)
    }

    /// Calls git log to list commit hashes for the repo in `repo_dir`, optionally on
    /// `query.branch`, optionally starting at the date specified by `query.since`, optionally
    /// ending at the date specified by `query.until`, optionally limited to max `query.number`
    /// commits
    fn git_log_commits(&self, repo_dir: &Path, query: &CommitQuery) -> Result<Vec<String>, Error> {
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!(
                "git --no-pager log {} {} {} {} --pretty=format:\"%H\"",
                match &query.branch {
                    Some(b) => {
                        // Since this is a no-checkout clone, we need to make sure we're referring to the
                        // branches starting with origin/ if it's just the branch name
                        if b.starts_with("origin/") {
                            format!("\'{}\'", b.replace("\'", "\\\'"))
                        }
                        else {
                            format!("\'origin/{}\'", b.replace("\'", "\\\'"))
                        }
                    },
                    None => String::from("")
                },
                match query.since { Some(s) => format!("--since \"{}\"", s), None => String::from("")},
                match query.until { Some(u) => format!("--until \"{}\"", u), None => String::from("")},
                match query.number { Some(n) => format!("-n \"{}\"", n), None => String::from("")},
            ))
            .output()?;

        if output.status.success() {
            let stdout: String = String::from_utf8_lossy(&*output.stdout).to_string();
            // Split the output on newlines to get the list of commits
            Ok(stdout.split_terminator("\n").map(String::from).collect())
        } else {
            Err(Error::Git(
                self.censor_git_error(String::from_utf8_lossy(&*output.stderr).to_string()),
            ))
        }
    }

    /// Calls git tags --list to list the last `query.number` (or fewer if there aren't that many)
    /// tags for the repo in `repo_dir`, optionally on `query. branch`
    fn git_tags_list(&self, repo_dir: &Path, query: &TagQuery) -> Result<Vec<String>, Error> {
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!(
                "git tag {} --list --sort -creatordate",
                match &query.branch {
                    Some(b) => {
                        // Since this is a no-checkout clone, we need to make sure we're referring to the
                        // branches starting with origin/ if it's just the branch name
                        if b.starts_with("origin/") {
                            format!("--merged \'{}\'", b.replace("\'", "\\\'"))
                        }
                        else {
                            format!("--merged \'origin/{}\'", b.replace("\'", "\\\'"))
                        }
                    },
                    None => String::from("")
                }
            ))
            .output()?;

        if output.status.success() {
            let stdout: String = String::from_utf8_lossy(&*output.stdout).to_string();
            // Split the output on newlines to get the list of commits
            Ok(stdout.split_terminator("\n").take(query.number).map(String::from).collect())
        } else {
            Err(Error::Git(
                self.censor_git_error(String::from_utf8_lossy(&*output.stderr).to_string()),
            ))
        }
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
                self.censor_git_error(String::from_utf8_lossy(&*output.stderr).to_string()),
            ))
        }
    }

    /// Runs git rev-parse on the git repo in `repo_dir` to get the commit hash for `git_ref`.
    /// Returns the commit hash if `git_ref` is a valid ref, otherwise returns Error::NotFound. If
    /// it fails for any other reason, returns an error.
    fn git_rev_parse_verify(&self, repo_dir: &Path, git_ref: &str) -> Result<String, Error> {
        // Run the command
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!(
                "git rev-parse --verify \'{}^{{commit}}\'",
                git_ref
            ))
            .output()?;

        // If the command was successful, return the commit hash
        if output.status.success() {
            Ok(String::from_utf8_lossy(&*output.stdout)
                .to_string()
                .trim()
                .to_string())
        } else {
            let stderr = String::from_utf8_lossy(&*output.stderr).to_string();
            // If stderr matches the message that git spits out if the commit is not valid, return
            // NotFound
            if stderr.trim() == "fatal: Needed a single revision" {
                return Err(Error::NotFound(git_ref.to_string()));
            }
            // Otherwise return an error
            Err(Error::Git(self.censor_git_error(stderr)))
        }
    }

    /// Calls git tag --points-at on the repo in `repo_dir` for `commit` and returns the resulting
    /// tags if successful, or an error if the command fails for some other reason
    fn git_tag_points_at(&self, repo_dir: &Path, commit: &str) -> Result<Vec<String>, Error> {
        let output = Command::new("sh")
            .current_dir(repo_dir)
            .arg("-c")
            .arg(format!("git tag --points-at \'{}\'", commit.replace("\'", "\\\'")))
            .output()?;

        if output.status.success() {
            let stdout: String = String::from_utf8_lossy(&*output.stdout).to_string();
            // Split the output on newlines to get the list of tags
            Ok(stdout.split_terminator("\n").map(String::from).collect())
        } else {
            Err(Error::Git(
                self.censor_git_error(String::from_utf8_lossy(&*output.stderr).to_string()),
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
                "git show --no-patch --no-notes --pretty='%cd' \'{}\'",
                commit.replace("\'", "\\\'")
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
                self.censor_git_error(String::from_utf8_lossy(&*output.stderr).to_string()),
            ))
        }
    }

    /// I'm not 100% sure if any of git's error messages could include credentials, so just in case,
    /// I'm gonna censor them
    fn censor_git_error(&self, mut error_message: String) -> String {
        // We're only checking for credentials if we actually have any
        if let Some(creds) = &self.private_github_config {
            error_message = error_message.replace(creds.client_id(), "*******").replace(creds.client_token(), "*******");
        }
        error_message
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::read_to_string;
    use std::str::FromStr;
    use tempfile::TempDir;
    use crate::unit_test_util::get_test_remote_github_repo;

    fn create_test_repo_manager() -> (GitRepoManager, TempDir) {
        let repo_manager_dir = TempDir::new().unwrap();
        let repo_manager = GitRepoManager{
            private_github_config: None,
            repo_cache_location: repo_manager_dir.path().to_str().unwrap().to_string()
        };

        (repo_manager, repo_manager_dir)
    }

    #[test]
    fn get_commit_and_tag_from_ref_success_commit() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_ref(
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

    #[test]
    fn get_commit_and_tag_from_ref_success_commit_no_tags() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_ref(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                &second_commit,
            )
            .expect("Error when checking for commit and tags");

        assert_eq!(commit, second_commit);
        assert_eq!(commit_date, second_date);
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn get_commit_and_tag_from_ref_success_tag() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_ref(
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

    #[test]
    fn get_commit_and_tags_from_ref_success_branch() {
        let (git_repo_temp_dir, _first_commit, second_commit, _first_date, second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_ref(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                "test_branch",
            )
            .expect("Error when checking for commit and tags");

        assert_eq!(commit, second_commit);
        assert_eq!(commit_date, second_date);
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn get_commit_and_tags_from_ref_success_short_hash() {
        let (git_repo_temp_dir, _first_commit, second_commit, _first_date, second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let (commit, tags, commit_date) = git_repo_manager
            .get_commit_and_tags_and_date_from_ref(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                &second_commit[..10],
            )
            .expect("Error when checking for commit and tags");

        assert_eq!(commit, second_commit);
        assert_eq!(commit_date, second_date);
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn get_commit_and_tag_from_ref_failure_not_found() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commit_or_tag = String::from("last");
        let error = git_repo_manager
            .get_commit_and_tags_and_date_from_ref(
                Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                &commit_or_tag,
            )
            .expect_err("No error when checking for commit and tags");

        assert!(matches!(error, Error::NotFound(commit_or_tag)));
    }

    #[test]
    fn get_commit_and_tag_from_ref_failure_no_software() {
        let (git_repo_temp_dir, first_commit, second_commit, first_date, second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let error = git_repo_manager
            .get_commit_and_tags_and_date_from_ref(
                Uuid::from_str("2d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
                "first",
            )
            .expect_err("No error when checking for commit and tags");

        assert!(matches!(error, Error::IO(_)));
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_since() {
        let (git_repo_temp_dir, _first_commit, second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: None,
                since: Some(NaiveDateTime::parse_from_str("2022-10-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()),
                until: None,
                number: None
            }
        ).unwrap();
        assert_eq!(commits, vec![second_commit])
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_until() {
        let (git_repo_temp_dir, first_commit, _second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: None,
                since: None,
                until: Some(NaiveDateTime::parse_from_str("2022-10-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()),
                number: None
            }
        ).unwrap();
        assert_eq!(commits, vec![first_commit])
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_branch() {
        let (git_repo_temp_dir, first_commit, _second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: Some(String::from("master")),
                since: None,
                until: None,
                number: None
            }
        ).unwrap();
        assert_eq!(commits, vec![first_commit])
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_branch_two_commits() {
        let (git_repo_temp_dir, first_commit, second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: Some(String::from("test_branch")),
                since: None,
                until: None,
                number: None
            }
        ).unwrap();
        assert_eq!(commits, vec![second_commit, first_commit])
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_number() {
        let (git_repo_temp_dir, _first_commit, second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: None,
                since: None,
                until: None,
                number: Some(1)
            }
        ).unwrap();
        assert_eq!(commits, vec![second_commit])
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_number_with_branch() {
        let (git_repo_temp_dir, first_commit, _second_commit, _first_date, _second_date) =
            crate::unit_test_util::get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: Some(String::from("master")),
                since: None,
                until: None,
                number: Some(1)
            }
        ).unwrap();
        assert_eq!(commits, vec![first_commit])
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_number_more_than_commits() {
        let (git_repo_temp_dir, first_commit, second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: None,
                since: None,
                until: None,
                number: Some(3)
            }
        ).unwrap();
        assert_eq!(commits, vec![second_commit, first_commit])
    }

    #[test]
    fn query_repo_for_commits_or_tags_success_tags() {
        let (git_repo_temp_dir, _first_commit, _second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let commits = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &TagQuery {
                branch: Some(String::from("master")),
                number: 1
            }
        ).unwrap();
        assert_eq!(commits, vec!["first"])
    }

    #[test]
    fn query_repo_for_commits_or_tags_failure_no_branch() {
        let (git_repo_temp_dir, _first_commit, _second_commit, _first_date, _second_date) =
            get_test_remote_github_repo();
        let (git_repo_manager, _manager_temp_dir) = create_test_repo_manager();
        git_repo_manager.download_git_repo(Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(), git_repo_temp_dir.to_str().unwrap()).unwrap();
        let query_error = git_repo_manager.query_repo_for_commits_or_tags(
            Uuid::from_str("6d80625b-5044-4aad-8d21-5d648371b52a").unwrap(),
            &CommitQuery {
                branch: Some(String::from("not_a_real_branch")),
                since: None,
                until: None,
                number: None
            }
        ).unwrap_err();
        assert!(matches!(query_error, Error::Git(_)));
    }

    #[test]
    fn censor_git_error_has_creds() {
        let git_repo_manager = GitRepoManager::new(
            Some(PrivateGithubAccessConfig::new(
                String::from("test_client_id"),
                String::from("test_client_token"),
                String::from(""),
                String::from(""),
                String::from("")),
            ),
            String::from("Repo dir doesn't matter for this test")
        );

        let censored_error = git_repo_manager.censor_git_error(String::from("This error message has credentials in it: test_client_id:test_client_token"));

        assert_eq!(censored_error, "This error message has credentials in it: *******:*******");
    }

    #[test]
    fn censor_git_error_no_creds() {
        let git_repo_manager = GitRepoManager::new(
            Some(PrivateGithubAccessConfig::new(
                String::from("test_client_id"),
                String::from("test_client_token"),
                String::from(""),
                String::from(""),
                String::from("")),
            ),
            String::from("Repo dir doesn't matter for this test")
        );

        let censored_error = git_repo_manager.censor_git_error(String::from("This error message has no credentials in it"));

        assert_eq!(censored_error, "This error message has no credentials in it");
    }
}
