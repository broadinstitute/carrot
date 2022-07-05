create table run_group(
    run_group_id uuid primary key DEFAULT uuid_generate_v4(),
    created_at timestamptz not null default current_timestamp
);

alter table run
    add column run_group_id uuid references run_group(run_group_id);

create index on run(run_group_id);

drop view if exists run_with_results_and_errors;

create view run_with_results_and_errors as
select run_id, test_id, run_group_id, name, status, test_input, test_options, eval_input,
       eval_options, test_cromwell_job_id, eval_cromwell_job_id, created_at, created_by,
       finished_at, results, errors
from run
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
     ) as results using (run_id);

create table run_group_is_from_github(
    run_group_id uuid primary key references run_group(run_group_id),
    owner text not null,
    repo text not null,
    issue_number integer not null,
    author text not null,
    base_commit text not null,
    head_commit text not null,
    test_input_key text,
    eval_input_key text,
    created_at timestamptz not null default current_timestamp
);