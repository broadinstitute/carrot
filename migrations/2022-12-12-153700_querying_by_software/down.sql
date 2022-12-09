drop view if exists run_group_with_metadata;
create view run_group_with_github as
select run_group_id, owner, repo, issue_number, author, base_commit, head_commit, test_input_key, eval_input_key,
       run_group.created_at as created_at
from run_group left join run_group_is_from_github using (run_group_id);

drop view if exists run_software_versions_with_identifiers;

alter table run
    add column run_group_id uuid references run_group(run_group_id);
create index on run(run_group_id);

update run
set run_group_id = subquery.run_group_id
from (select run_id, run_group_id from run_in_group) as subquery
where run.run_id = subquery.run_id;

drop view run_with_results_and_errors;
create view run_with_results_and_errors as
select run_id, test_id, run_group_id, name, status, test_wdl, tw.hash as test_wdl_hash, test_wdl_dependencies, td.hash
        as test_wdl_dependencies_hash, eval_wdl, ew.hash as eval_wdl_hash, eval_wdl_dependencies, ed.hash as
        eval_wdl_dependencies_hash, test_input, test_options, eval_input, eval_options,
        test_cromwell_job_id, eval_cromwell_job_id, run.created_at as created_at, created_by, finished_at, results,
        errors
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
     ) as results using (run_id);

drop table if exists run_in_group;
drop table if exists run_group_is_from_query;



