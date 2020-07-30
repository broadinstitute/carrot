drop table if exists run_software_version;
drop table if exists software_build;
drop table if exists software_version;
drop table if exists template_software;
drop table if exists software;

-- Removing the 'building' status is a bit involved because the enum needs to be replaced
update run set status = 'submitted' where status = 'building';
alter type run_status_enum rename to old_run_status_enum;
create type run_status_enum as enum('submitted', 'running', 'succeeded', 'failed', 'aborted', 'starting', 'queued_in_cromwell', 'waiting_for_queue_space');
alter table run alter column status type run_status_enum using (status::text::run_status_enum);
drop type old_run_status_enum;

drop type build_status_enum;


