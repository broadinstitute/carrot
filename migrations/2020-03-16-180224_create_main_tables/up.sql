create table pipeline(
	pipeline_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	name text not null unique,
	description text,
	created_at timestamptz not null default current_timestamp,
	created_by text
);

create table template(
	template_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	pipeline_id UUID NOT NULL,
	name text not null unique,
	description text,
	test_wdl text not null,
	eval_wdl text not null,
	created_at timestamptz not null default current_timestamp,
	created_by text
);

create table test(
	test_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	template_id UUID NOT NULL,
	name text not null unique,
	description text,
	test_input_defaults jsonb,
	eval_input_defaults jsonb,
	created_at timestamptz not null default current_timestamp,
	created_by text
);