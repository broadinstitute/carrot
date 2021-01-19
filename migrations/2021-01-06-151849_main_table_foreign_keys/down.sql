alter table template drop constraint template_pipeline_id_fkey;
alter table test drop constraint test_template_id_fkey;
alter table run drop constraint run_test_id_fkey;
alter table run_result drop constraint run_result_run_id_fkey;
alter table run_result drop constraint run_result_result_id_fkey;
alter table template_result drop constraint template_result_template_id_fkey;
alter table template_result drop constraint template_result_result_id_fkey;
