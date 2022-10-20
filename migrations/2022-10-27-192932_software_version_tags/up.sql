create table software_version_tag(
    software_version_id uuid not null references software_version(software_version_id),
    tag text not null,
    created_at timestamptz not null default current_timestamp,
    primary key(software_version_id, tag)
);

alter table software_version
    add column commit_date timestamptz not null default to_timestamp(0);

create view software_version_with_tags as
    select software_version_id, software_id, commit, commit_date, coalesce(array_agg(tag) filter (where tag is not null), '{}') as tags, software_version.created_at as created_at
    from software_version left outer join software_version_tag using (software_version_id)
    group by software_version_id;