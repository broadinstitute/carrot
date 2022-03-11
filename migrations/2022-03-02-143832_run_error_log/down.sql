drop view if exists run_with_results_and_errors;
drop table if exists run_error;

create view run_id_with_results as
select run_id, json_object_agg(name, value) as results
from run_result inner join result using (result_id)
group by run_id;