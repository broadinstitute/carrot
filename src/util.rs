pub struct SortClause {
    pub key: String,
    pub ascending: bool,
}

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
