alter type run_status_enum rename to old_run_status_enum;
create type run_status_enum as enum(
    'aborted',
    'building',
    'created',
    'failed',
    'queued_in_cromwell',
    'running',
    'starting',
    'submitted',
    'succeeded',
    'waiting_for_queue_space'
);

alter table run
    alter column status
        set data type run_status_enum
        using (
            case status::text
                when 'test_aborted' then 'aborted'
                when 'eval_aborted' then 'aborted'
                when 'test_failed' then 'failed'
                when 'eval_failed' then 'failed'
                when 'carrot_failed' then 'failed'
                when 'build_failed' then 'failed'
                when 'test_queued_in_cromwell' then 'queued_in_cromwell'
                when 'eval_queued_in_cromwell' then 'queued_in_cromwell'
                when 'test_running' then 'running'
                when 'eval_running' then 'running'
                when 'test_starting' then 'starting'
                when 'eval_starting' then 'starting'
                when 'test_submitted' then 'submitted'
                when 'eval_submitted' then 'submitted'
                when 'test_waiting_for_queue_space' then 'waiting_for_queue_space'
                when 'eval_waiting_for_queue_space' then 'waiting_for_queue_space'
                else status::text
            end
        )::run_status_enum;

drop type old_run_status_enum;

alter table run
    drop column eval_cromwell_job_id;

alter table run
    rename column test_cromwell_job_id to cromwell_job_id;