create table report (
    report_id uuid primary key DEFAULT uuid_generate_v4(),
    name text not null unique,
    description text,
    metadata jsonb not null,
    created_at timestamptz not null default current_timestamp,
    created_by text
);

create table section (
    section_id uuid primary key DEFAULT uuid_generate_v4(),
    name text not null unique,
    description text,
    contents jsonb not null,
    created_at timestamptz not null default current_timestamp,
    created_by text
);

create table report_section (
    report_id uuid not null references report(report_id),
    section_id uuid not null references section(section_id),
    position integer not null,
    created_at timestamptz not null default current_timestamp,
    created_by text,
    primary key (report_id, section_id)
);

create table template_report (
    template_id uuid not null references template(template_id),
    report_id uuid not null references report(report_id),
    input_map jsonb not null,
    created_at timestamptz not null default current_timestamp,
    created_by text,
    primary key (template_id, report_id)
);

create type report_status_enum as enum('aborted', 'created', 'expired', 'failed', 'queued_in_cromwell', 'running', 'starting', 'submitted', 'succeeded', 'waiting_for_queue_space');

create table run_report (
    run_id uuid not null references run(run_id),
    report_id uuid not null references report(report_id),
    status report_status_enum not null,
    cromwell_job_id text,
    results jsonb,
    created_at timestamptz not null default current_timestamp,
    created_by text,
    finished_at timestamptz,
    primary key (run_id, report_id)
);