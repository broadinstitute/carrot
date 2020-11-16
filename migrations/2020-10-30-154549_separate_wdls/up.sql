alter type run_status_enum rename to old_run_status_enum;
create type run_status_enum as enum(
    'build_failed',
    'building',
    'carrot_failed',
    'created',
    'eval_aborted',
    'eval_aborting',
    'eval_failed',
    'eval_queued_in_cromwell',
    'eval_running',
    'eval_starting',
    'eval_submitted',
    'eval_waiting_for_queue_space',
    'succeeded',
    'test_aborted',
    'test_aborting',
    'test_failed',
    'test_queued_in_cromwell',
    'test_running',
    'test_starting',
    'test_submitted',
    'test_waiting_for_queue_space'
);

alter table run
    alter column status
    set data type run_status_enum
    using (
        case status::text
            when 'aborted' then 'test_aborted'
            when 'failed' then 'carrot_failed'
            when 'queued_in_cromwell' then 'eval_queued_in_cromwell'
            when 'running' then 'eval_running'
            when 'starting' then 'eval_starting'
            when 'submitted' then 'eval_submitted'
            when 'waiting_for_queue_space' then 'eval_waiting_for_queue_space'
            else status::text
        end
    )::run_status_enum;

drop type old_run_status_enum;

alter table run
    add eval_cromwell_job_id text;

alter table run
    rename column cromwell_job_id to test_cromwell_job_id;