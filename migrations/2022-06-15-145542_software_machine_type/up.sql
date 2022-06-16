create type machine_type_enum as enum('n1-highcpu-8', 'n1-highcpu-32', 'standard');

alter table software
    add column machine_type machine_type_enum not null default 'standard';