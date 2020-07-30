create type entity_type_enum as enum('pipeline', 'template', 'test');

create table subscription(
    subscription_id uuid primary key DEFAULT uuid_generate_v4(),
    entity_type entity_type_enum not null,
    entity_id uuid not null,
    email text not null,
    created_at timestamptz not null default current_timestamp
);

