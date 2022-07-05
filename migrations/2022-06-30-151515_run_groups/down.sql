drop table if exists run_group_is_from_github;

drop view if exists run_with_results_and_errors;

create view run_with_results_and_errors as
select run_id, test_id, name, status, test_input, test_options, eval_input,
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

alter table run
    drop column if exists run_group_id;

drop table if exists run_group;