create table run_is_from_github (
    run_id uuid primary key references run(run_id),
    owner text not null,
    repo text not null,
    issue_number integer not null,
    author text not null,
    created_at timestamptz not null default current_timestamp
);

