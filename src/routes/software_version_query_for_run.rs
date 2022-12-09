//! Defines a struct and functions for handling parsing

use std::fmt;
use crate::util::git_repos;
use chrono::NaiveDateTime;
use diesel::PgConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::models::software::SoftwareData;
use crate::util::git_repos::GitRepoManager;

/// Represents the types of software version queries a user can use when querying for a list of
/// runs
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SoftwareVersionQueryForRun {
    List {
        name: String,
        commits_and_tags: Vec<String>
    },
    Count {
        name: String,
        count: u32,
        branch: Option<String>,
        #[serde(default)]
        tags_only: bool
    },
    Dates {
        name: String,
        from: Option<NaiveDateTime>,
        to: Option<NaiveDateTime>,
        branch: Option<String>
    }
}

#[derive(Debug)]
pub enum Error {
    Git(git_repos::Error),
    DB(diesel::result::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Git(e) => write!(f, "SoftwareVersionQueryForRun Error Git {}", e),
            Error::DB(e) => write!(f, "SoftwareVersionQueryForRun Error DB {}", e),
        }
    }
}

impl From<git_repos::Error> for Error {
    fn from(e: git_repos::Error) -> Error {
        Error::Git(e)
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}

impl SoftwareVersionQueryForRun {

    /// Get a vector of strings in the format "{software_name}|{commit_or_tag}" matching the query
    /// parameters defined by `self`
    ///
    /// In the case of `self::List`, builds the strings from the `name` and list of
    /// `commits_and_tags`.
    /// In the case of `self::Count`, uses `conn` to retrieve the software matching `name` and uses
    /// `git_repo_manager` to get the list of the latest `count` commits, optionally from `branch`,
    /// and uses that to build the list of strings
    /// In the case of `self::Dates`, uses `conn` to retrieve the software matching `name` and uses
    /// `git_repo_manager` to get the list of commits optionally after `to`, optionally before
    /// `from`, optionally from `branch` and uses that to build the list of strings
    pub fn get_strings_for_query(
        &self,
        conn: &PgConnection,
        git_repo_manager: &git_repos::GitRepoManager,
    ) -> Result<Vec<String>, Error> {
        let strings_for_query: Vec<String> = match self {
            SoftwareVersionQueryForRun::List {name, commits_and_tags} => {
                // Fill list by combining software name with commits and tags (the format the query
                // will expect)
                let mut strings_for_query: Vec<String> = Vec::new();
                for commit_or_tag in commits_and_tags {
                    strings_for_query.push([&name, "|", commit_or_tag].join(""));
                }
                strings_for_query
            },
            SoftwareVersionQueryForRun::Count {name, count, branch, tags_only} => {
                // Get software so we can pass its id to git_repo_manager to retrieve the commits
                let software: SoftwareData = SoftwareData::find_by_name_ignore_case(conn, name)?;
                // Get the list of commits/tags from the git repo
                // The type of query we're doing is dependent on whether we want tags only
                let commits = if *tags_only {
                    let query = git_repos::TagQuery {
                        branch: match branch {Some(b) => Some(b.to_string()), None => None},
                        number: *count as usize
                    };
                    SoftwareVersionQueryForRun::query_repo_and_download_if_not_present(git_repo_manager, &query, software.software_id, &software.repository_url)?
                }
                else {
                    let query = git_repos::CommitQuery {
                        branch: match branch {Some(b) => Some(b.to_string()), None => None},
                        since: None,
                        until: None,
                        number: Some(*count)
                    };
                    SoftwareVersionQueryForRun::query_repo_and_download_if_not_present(git_repo_manager, &query, software.software_id, &software.repository_url)?
                };
                // Fill list by combining software name with commits and tags (the format the query
                // will expect)
                let mut strings_for_query: Vec<String> = Vec::new();
                for commit in commits {
                    strings_for_query.push([&name, "|", &commit].join(""));
                }
                strings_for_query
            },
            SoftwareVersionQueryForRun::Dates {name, from, to, branch} => {
                // Get software so we can pass its id to git_repo_manager to retrieve the commits
                let software: SoftwareData = SoftwareData::find_by_name_ignore_case(conn, name)?;
                // Get the list of commits from the git repo
                let commits_query = git_repos::CommitQuery{
                    branch: match branch {Some(b) => Some(b.to_string()), None => None},
                    since: match from {Some(f) => Some(f.clone()), None => None},
                    until: match to {Some(t) => Some(t.clone()), None => None},
                    number: None
                };
                let commits: Vec<String> = SoftwareVersionQueryForRun::query_repo_and_download_if_not_present(git_repo_manager, &commits_query, software.software_id, &software.repository_url)?;
                // Fill list by combining software name with commits and tags (the format the query
                // will expect)
                let mut strings_for_query: Vec<String> = Vec::new();
                for commit in commits {
                    strings_for_query.push([&name, "|", &commit].join(""));
                }
                strings_for_query
            }
        };

        Ok(strings_for_query)
    }

    /// Queries the repo for the software specified by `software_id` with parameters specified in
    /// `query`. Attempts to clone the repo from `repository_url` if it does not appear to
    /// have been cached yet. Returns the list of found commits if successful or an error if
    /// unsuccessful
    fn query_repo_and_download_if_not_present(git_repo_manager: &GitRepoManager, query: &impl git_repos::GitQuery, software_id: Uuid, repository_url: &str) -> Result<Vec<String>, Error> {
        match git_repo_manager.query_repo_for_commits_or_tags(software_id, query) {
            Ok(found_commits) => Ok(found_commits),
            Err(git_repos::Error::IO(e)) => {
                // If we get an IO NotFound error, that indicates we almost definitely haven't
                // cloned the repo yet, so we'll clone and try again
                if e.kind() == std::io::ErrorKind::NotFound {
                    git_repo_manager.download_git_repo(
                        software_id,
                        repository_url,
                    )?;
                    // Try again
                    Ok(git_repo_manager.query_repo_for_commits_or_tags(software_id, query)?)
                }
                // If it was a different kind of error, return it
                else {
                    return Err(Error::Git(git_repos::Error::IO(e)))
                }
            }
            Err(e) => return Err(Error::Git(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use diesel::PgConnection;
    use tempfile::TempDir;
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::routes::software_version_query_for_run::{Error, SoftwareVersionQueryForRun};
    use crate::unit_test_util::{get_test_db_connection, get_test_remote_github_repo};
    use crate::util::git_repos::GitRepoManager;

    fn create_test_repo_manager() -> (GitRepoManager, TempDir) {
        let repo_manager_dir = TempDir::new().unwrap();
        let repo_manager = GitRepoManager::new(None, repo_manager_dir.path().to_str().unwrap().to_string());

        (repo_manager, repo_manager_dir)
    }

    fn create_test_software_with_repo(conn: &PgConnection) -> (SoftwareData, String, String, NaiveDateTime, NaiveDateTime) {
        let (test_repo_dir, commit1, commit2, commit1_date, commit2_date) = get_test_remote_github_repo();
        let software = SoftwareData::create(conn, NewSoftware {
            name: "TestSoftware".to_string(),
            description: None,
            repository_url: String::from(test_repo_dir.as_path().to_str().unwrap()),
            machine_type: None,
            created_by: None
        }).unwrap();

        (software, commit1, commit2, commit1_date, commit2_date)
    }

    #[test]
    fn get_strings_for_query_success_list() {
        let conn = get_test_db_connection();
        let (git_repo_manager, _repo_dir) = create_test_repo_manager();

        let test_query_for_run = SoftwareVersionQueryForRun::List {
            name: String::from("TestSoftware"),
            commits_and_tags: vec![
                String::from("first"),
                String::from("2009358fd05c3fb67117d909f8e4f93f19239d0c")
            ]
        };

        let strings = test_query_for_run.get_strings_for_query(&conn, &git_repo_manager).unwrap();
        assert_eq!(strings, vec!["TestSoftware|first", "TestSoftware|2009358fd05c3fb67117d909f8e4f93f19239d0c"]);
    }

    #[test]
    fn get_strings_for_query_success_count() {
        let conn = get_test_db_connection();
        let (git_repo_manager, _repo_dir) = create_test_repo_manager();
        let (software, commit1, _, _, _) = create_test_software_with_repo(&conn);
        git_repo_manager.download_git_repo(software.software_id, &software.repository_url).unwrap();

        let test_query_for_run = SoftwareVersionQueryForRun::Count {
            name: String::from("TestSoftware"),
            count: 1,
            branch: Some(String::from("master")),
            tags_only: false
        };

        let strings = test_query_for_run.get_strings_for_query(&conn, &git_repo_manager).unwrap();
        assert_eq!(strings, vec![format!("TestSoftware|{}", commit1)]);
    }

    #[test]
    fn get_strings_for_query_success_count_need_download() {
        let conn = get_test_db_connection();
        let (git_repo_manager, _repo_dir) = create_test_repo_manager();
        let (software, commit1, _, _, _) = create_test_software_with_repo(&conn);

        let test_query_for_run = SoftwareVersionQueryForRun::Count {
            name: String::from("TestSoftware"),
            count: 1,
            branch: Some(String::from("master")),
            tags_only: false
        };

        let strings = test_query_for_run.get_strings_for_query(&conn, &git_repo_manager).unwrap();
        assert_eq!(strings, vec![format!("TestSoftware|{}", commit1)]);
    }

    #[test]
    fn get_strings_for_query_success_dates() {
        let conn = get_test_db_connection();
        let (git_repo_manager, _repo_dir) = create_test_repo_manager();
        let (software, _, commit2, _, _) = create_test_software_with_repo(&conn);
        git_repo_manager.download_git_repo(software.software_id, &software.repository_url).unwrap();

        let test_query_for_run = SoftwareVersionQueryForRun::Dates {
            name: String::from("TestSoftware"),
            to: Some(NaiveDateTime::parse_from_str("2022-12-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()),
            from: Some(NaiveDateTime::parse_from_str("2022-10-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()),
            branch: None
        };

        let strings = test_query_for_run.get_strings_for_query(&conn, &git_repo_manager).unwrap();
        assert_eq!(strings, vec![format!("TestSoftware|{}", commit2)]);
    }

    #[test]
    fn get_strings_for_query_success_tags() {
        let conn = get_test_db_connection();
        let (git_repo_manager, _repo_dir) = create_test_repo_manager();
        let (software, _, _, _, _) = create_test_software_with_repo(&conn);
        git_repo_manager.download_git_repo(software.software_id, &software.repository_url).unwrap();

        let test_query_for_run = SoftwareVersionQueryForRun::Count {
            name: String::from("TestSoftware"),
            branch: None,
            count: 1,
            tags_only: true

        };

        let strings = test_query_for_run.get_strings_for_query(&conn, &git_repo_manager).unwrap();
        assert_eq!(strings, vec!["TestSoftware|first"]);
    }

    #[test]
    fn get_strings_for_query_failure_no_software() {
        let conn = get_test_db_connection();
        let (git_repo_manager, _repo_dir) = create_test_repo_manager();

        let test_query_for_run = SoftwareVersionQueryForRun::Dates {
            name: String::from("TestSoftware"),
            to: Some(NaiveDateTime::parse_from_str("2022-12-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()),
            from: Some(NaiveDateTime::parse_from_str("2022-10-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()),
            branch: None
        };

        let error = test_query_for_run.get_strings_for_query(&conn, &git_repo_manager).unwrap_err();
        assert!(matches!(error, Error::DB(diesel::NotFound)));
    }
}