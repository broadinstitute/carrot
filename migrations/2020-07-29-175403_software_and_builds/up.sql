create type build_status_enum as enum('aborted', 'expired', 'failed', 'queued_in_cromwell', 'running', 'starting', 'submitted', 'succeeded', 'waiting_for_queue_space');

-- Adding the 'building' status is a bit involved because the enum needs to be replaced
-- (Diesel runs these in a transaction, and postgres won't let you add values to a type in a transaction)
-- (Also taking this opportunity to reorder the run_status_enum values)
alter type run_status_enum rename to old_run_status_enum;
create type run_status_enum as enum('aborted', 'building', 'failed', 'queued_in_cromwell', 'running', 'starting', 'submitted', 'succeeded', 'waiting_for_queue_space');
alter table run alter column status type run_status_enum using (status::text::run_status_enum);
drop type old_run_status_enum;

create table software(
    software_id uuid primary key DEFAULT uuid_generate_v4(),
    name text not null unique,
    description text,
    repository_url text not null,
    created_at timestamptz not null default current_timestamp,
    created_by text
);

create table template_software(
    template_id uuid not null references template(template_id),
    software_id uuid not null references software(software_id),
    image_key text not null,
    created_at timestamptz not null default current_timestamp,
    created_by text,
    primary key (template_id, software_id)
);

create table software_version(
      software_version_id uuid primary key default uuid_generate_v4(),
      software_id uuid not null references software(software_id),
      commit text not null,
      created_at timestamptz not null default current_timestamp
);

create table software_build(
     software_build_id uuid primary key default uuid_generate_v4(),
     software_version_id uuid not null references software_version(software_version_id),
     cromwell_job_id text,
     status build_status_enum not null,
     image_url text,
     created_at timestamptz not null default current_timestamp,
     finished_at timestamptz
);

create table run_software_version(
    run_id uuid not null references run(run_id),
    software_version_id uuid not null references software_version(software_version_id),
    created_at timestamptz not null default current_timestamp,
    primary key (run_id, software_version_id)
);

