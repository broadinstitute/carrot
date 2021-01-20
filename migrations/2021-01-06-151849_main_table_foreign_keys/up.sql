alter table template add constraint template_pipeline_id_fkey foreign key (pipeline_id) references pipeline(pipeline_id);
alter table test add constraint test_template_id_fkey foreign key (template_id) references template(template_id);
alter table run add constraint run_test_id_fkey foreign key (test_id) references test(test_id);
alter table run_result add constraint run_result_run_id_fkey foreign key (run_id) references run(run_id);
alter table run_result add constraint run_result_result_id_fkey foreign key (result_id) references result(result_id);
alter table template_result add constraint template_result_template_id_fkey foreign key (template_id) references template(template_id);
alter table template_result add constraint template_result_result_id_fkey foreign key (result_id) references result(result_id);
