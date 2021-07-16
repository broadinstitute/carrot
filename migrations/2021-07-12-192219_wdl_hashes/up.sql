create table wdl_hash (
     location text not null,
     hash bytea not null,
     created_at timestamptz not null default current_timestamp,
     primary key (location, hash)
);

create index hash_idx on wdl_hash (hash);

