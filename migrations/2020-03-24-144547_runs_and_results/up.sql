create type run_status_enum as enum('created', 'running', 'completed', 'failed');

create type result_type_enum as enum('numeric', 'file');

create table run(
    run_id uuid primary key DEFAULT uuid_generate_v4(),
    test_id uuid not null,
    name text not null unique,
    status run_status_enum not null,
    test_input jsonb not null,
    eval_input jsonb not null,
    cromwell_job_id text,
    created_at timestamptz not null default current_timestamp,
    created_by text,
    finished_at timestamptz
);

create table result(
    result_id uuid primary key DEFAULT uuid_generate_v4(),
    name text not null unique,
    result_type result_type_enum not null,
    description text,
    created_at timestamptz not null default current_timestamp,
    created_by text
);

create table run_result_file(
    run_id uuid not null,
    result_id uuid not null,
    uri text not null,
    created_at timestamptz not null default current_timestamp,
    primary key (run_id, result_id)
);

create table run_result_numeric(
    run_id uuid not null,
    result_id uuid not null,
    value double precision not null,
    created_at timestamptz not null default current_timestamp,
    primary key (run_id, result_id)
);

create table template_result (
    template_id uuid not null,
    result_id uuid not null,
    result_key text not null,
    created_at timestamptz not null default current_timestamp,
    created_by text,
    primary key (template_id, result_id)
);