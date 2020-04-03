//! Provides utility functionality for data handling within the project
//!
//! Should and will probably be moved to a module where it is relevant, in favor of having a
//! forever-growing util module

/// Defines a sort clause to be used in DB queries
pub struct SortClause {
    pub key: String,
    pub ascending: bool,
}

/// Parses the sort string for a query sent to the REST API
///
/// Expects sort strings to be comma-separated lists of sort keys, optionally enclosed in asc() or
/// desc().  For example, asc(name),desc(created_at),pipeline_id
pub fn parse_sort_string(sort_string: String) -> Vec<SortClause> {
    let mut sort_clauses = Vec::new();

    for clause in sort_string.split(",") {
        if clause.starts_with("asc(") {
            sort_clauses.push(SortClause {
                key: String::from(clause.trim_start_matches("asc(").trim_end_matches(")")),
                ascending: true,
            });
        } else if clause.starts_with("desc(") {
            sort_clauses.push(SortClause {
                key: String::from(clause.trim_start_matches("desc(").trim_end_matches(")")),
                ascending: false,
            });
        } else {
            sort_clauses.push(SortClause {
                key: String::from(clause),
                ascending: true,
            });
        }
    }

    sort_clauses
}
