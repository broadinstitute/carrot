alter table run
    add column test_wdl text,
    add column test_wdl_dependencies text,
    add column eval_wdl text,
    add column eval_wdl_dependencies text;

update run
    set test_wdl = test_with_wdls.test_wdl,
        test_wdl_dependencies = test_with_wdls.test_wdl_dependencies,
        eval_wdl = test_with_wdls.eval_wdl,
        eval_wdl_dependencies = test_with_wdls.eval_wdl_dependencies
    from (
         select test_id, test_wdl, test_wdl_dependencies, eval_wdl, eval_wdl_dependencies
         from test inner join template using (template_id)
    ) test_with_wdls
    where run.test_id = test_with_wdls.test_id;

alter table run
    alter column test_wdl set not null,
    alter column eval_wdl set not null;

create index wdl_hash_location_idx on wdl_hash(location);

drop view if exists run_with_results_and_errors;

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