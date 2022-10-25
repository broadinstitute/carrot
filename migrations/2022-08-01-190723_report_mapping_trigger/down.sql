drop view if exists run_group_with_github;

alter table template_report
    drop constraint template_report_pkey cascade,
    add primary key(template_id, report_id);

alter table template_report
    drop column if exists report_trigger;

drop type if exists report_trigger_enum;

alter table if exists report_map
    rename to run_report;

alter table run_report
    drop constraint run_report_pkey;
alter table run_report
    rename column entity_id to run_id;
alter table run_report
    add constraint run_report_run_id_fkey foreign key (run_id) references run (run_id);
alter table run_report
    drop column entity_type;
alter table run_report
    add primary key (run_id, report_id);

drop type if exists reportable_enum;
