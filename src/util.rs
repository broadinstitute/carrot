//! Provides utility functionality for data handling within the project
//!
//! Should and will probably be moved to a module where it is relevant, in favor of having a
//! forever-growing util module

use std::process::Command;

/// Defines a sort clause to be used in DB queries
#[derive(PartialEq, Debug)]
pub struct SortClause {
    pub key: String,
    pub ascending: bool,
}

/// Parses the sort string for a query sent to the REST API
///
/// Expects sort strings to be comma-separated lists of sort keys, optionally enclosed in asc() or
/// desc().  For example, asc(name),desc(created_at),pipeline_id
pub fn parse_sort_string(sort_string: &str) -> Vec<SortClause> {
    let mut sort_clauses = Vec::new();

    for clause in sort_string.split(",") {
        let clause = clause.trim();
        if clause.starts_with("asc(") {
            sort_clauses.push(SortClause {
                key: String::from(
                    clause
                        .trim_start_matches("asc(")
                        .trim_end_matches(")")
                        .trim(),
                ),
                ascending: true,
            });
        } else if clause.starts_with("desc(") {
            sort_clauses.push(SortClause {
                key: String::from(
                    clause
                        .trim_start_matches("desc(")
                        .trim_end_matches(")")
                        .trim(),
                ),
                ascending: false,
            });
        } else if !clause.is_empty() {
            sort_clauses.push(SortClause {
                key: String::from(clause),
                ascending: true,
            });
        }
    }

    sort_clauses
}

/// Checks where the remote git repo specified by `url` exists
///
/// Uses the `git ls-remote` command to check the specified url for a git repo.  Returns Ok(true)
/// if the command is successful, and Ok(false) if it fails.  Returns an error if there is some
/// error trying to execute the command
pub async fn git_repo_exists(url: &str) -> Result<bool, std::io::Error> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("git ls-remote {}", url))
        .output()?;

    if output.status.success() {
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_sort_string_empty() {
        assert_eq!(parse_sort_string(""), Vec::new());
    }

    #[test]
    fn parse_sort_string_whitespace() {
        assert_eq!(
            parse_sort_string("  \n\r\t\u{000B}\u{000C}\u{0085}\u{2028}\u{2029}"),
            Vec::new()
        );
    }

    #[test]
    fn parse_sort_string_middle_whitespace() {
        let sort = parse_sort_string("asc(name), ,version");
        assert_eq!(
            sort[0],
            SortClause {
                key: String::from("name"),
                ascending: true
            }
        );
        assert_eq!(
            sort[1],
            SortClause {
                key: String::from("version"),
                ascending: true
            }
        );
    }

    #[test]
    fn parse_sort_string_starting_whitespace() {
        let sort = parse_sort_string(" ,desc(description),version");
        assert_eq!(
            sort[0],
            SortClause {
                key: String::from("description"),
                ascending: false
            }
        );
        assert_eq!(
            sort[1],
            SortClause {
                key: String::from("version"),
                ascending: true
            }
        );
    }

    #[test]
    fn parse_sort_string_ending_whitespace() {
        let sort = parse_sort_string("asc(name),desc(description), ");
        assert_eq!(
            sort[0],
            SortClause {
                key: String::from("name"),
                ascending: true
            }
        );
        assert_eq!(
            sort[1],
            SortClause {
                key: String::from("description"),
                ascending: false
            }
        );
    }

    #[test]
    fn parse_sort_string_normal() {
        let sort = parse_sort_string("asc(name),desc(description),version");
        assert_eq!(
            sort[0],
            SortClause {
                key: String::from("name"),
                ascending: true
            }
        );
        assert_eq!(
            sort[1],
            SortClause {
                key: String::from("description"),
                ascending: false
            }
        );
        assert_eq!(
            sort[2],
            SortClause {
                key: String::from("version"),
                ascending: true
            }
        );
    }

    #[actix_rt::test]
    async fn git_repo_exists_true() {
        let test = git_repo_exists("git://github.com/broadinstitute/gatk.git")
            .await
            .expect("Error when checking if git repo exists");

        assert!(test);
    }
    #[actix_rt::test]
    async fn git_repo_exists_false() {
        let test = git_repo_exists("git://example.com/example/project.git")
            .await
            .expect("Error when checking if git repo exists");

        assert!(!test);
    }
}
