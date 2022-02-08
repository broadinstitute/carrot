alter table test
    add column test_option_defaults jsonb,
    add column eval_option_defaults jsonb;

alter table run
    add column test_options jsonb,
    add column eval_options jsonb;