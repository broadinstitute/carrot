create type report_trigger_enum as enum('pr', 'single');

alter table template_report
    add column report_trigger report_trigger_enum not null default 'single';

alter table template_report
    drop constraint template_report_pkey cascade,
    add primary key(template_id, report_id, report_trigger);

create type reportable_enum as enum('run', 'run_group');

alter table run_report
    drop constraint run_report_run_id_fkey;
alter table run_report
    drop constraint run_report_pkey;
alter table run_report
    rename column run_id to entity_id;
alter table run_report
    add column entity_type reportable_enum not null default 'run';
alter table run_report
    add primary key (entity_type, entity_id, report_id);
alter table run_report
    rename to report_map;

create view run_group_with_github as
select run_group_id, owner, repo, issue_number, author, base_commit, head_commit, test_input_key, eval_input_key,
       run_group.created_at as created_at
from run_group left join run_group_is_from_github using (run_group_id);