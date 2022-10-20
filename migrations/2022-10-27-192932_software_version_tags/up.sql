create table software_version_tag(
    software_version_id uuid not null references software_version(software_version_id),
    tag text not null,
    created_at timestamptz not null default current_timestamp,
    primary key(software_version_id, tag)
);