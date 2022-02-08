alter table test
    drop column test_option_defaults,
    drop column eval_option_defaults;

alter table run
    drop column test_options,
    drop column eval_options;