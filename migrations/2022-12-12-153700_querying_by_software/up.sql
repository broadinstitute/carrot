create view run_software_versions_with_identifiers as
select run_id, software.name || '|' || commit as "software_with_identifier" from run_software_version inner join software_version using (software_version_id) inner join software using (software_id)
union
select run_id, software.name || '|' || tag as "software_with_identifier" from run_software_version inner join software_version using (software_version_id) inner join software using (software_id) inner join software_version_tag using (software_version_id);

create table run_in_group(
    run_id uuid not null references run(run_id),
    run_group_id uuid not null references run_group(run_group_id),
    created_at timestamptz not null default current_timestamp,
    primary key(run_id, run_group_id)
);
create index on run_in_group(run_id);
create index on run_in_group(run_group_id);

insert into run_in_group(run_id, run_group_id)
select run_id, run_group_id from run where run_group_id is not null;

create table run_group_is_from_query(
    run_group_id uuid not null unique references run_group(run_group_id),
    query jsonb not null,
    created_at timestamptz not null default current_timestamp
);

drop view run_with_results_and_errors;
create view run_with_results_and_errors as
select run_id, test_id, coalesce(run_group_ids, '{}') as run_group_ids, name, status, test_wdl, tw.hash as
        test_wdl_hash, test_wdl_dependencies, td.hash as test_wdl_dependencies_hash, eval_wdl, ew.hash as eval_wdl_hash,
        eval_wdl_dependencies, ed.hash as eval_wdl_dependencies_hash, test_input, test_options, eval_input,
        eval_options, test_cromwell_job_id, eval_cromwell_job_id, run.created_at as created_at, created_by, finished_at,
        results, errors
from run
         left join wdl_hash tw on test_wdl = tw.location
         left join wdl_hash td on test_wdl_dependencies = td.location
         left join wdl_hash ew on eval_wdl = ew.location
         left join wdl_hash ed on eval_wdl_dependencies = ed.location
         left join
     (
         select run_id, jsonb_agg(to_char(created_at, 'YYYY-MM-DD HH24:MI:SS.MS') || ': ' || error) as errors
         from run_error
         group by run_id
     ) as errors using (run_id)
         left join
     (
         select run_id, jsonb_object_agg(name, value) as results
         from run_result inner join result using (result_id)
         group by run_id
     ) as results using (run_id)
    left join (select run_id, array_agg(run_group_id) as run_group_ids from run_in_group group by run_id)
        as run_groups using (run_id);

alter table run
    drop column if exists run_group_id;

drop view if exists run_group_with_github;

create view run_group_with_metadata as
select run_group_id, owner, repo, issue_number, author, base_commit, head_commit, test_input_key, eval_input_key,
       run_group_is_from_github.created_at as github_created_at, query,
       run_group_is_from_query.created_at as query_created_at, run_group.created_at as created_at
from run_group
    left join run_group_is_from_github using (run_group_id)
    left join run_group_is_from_query using (run_group_id);

