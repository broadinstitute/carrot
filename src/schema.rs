table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    pipeline (pipeline_id) {
        pipeline_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    result (result_id) {
        result_id -> Uuid,
        name -> Text,
        result_type -> Result_type_enum,
        description -> Nullable<Text>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    run (run_id) {
        run_id -> Uuid,
        test_id -> Uuid,
        name -> Text,
        status -> Run_status_enum,
        test_input -> Jsonb,
        eval_input -> Jsonb,
        cromwell_job_id -> Nullable<Text>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
        finished_at -> Nullable<Timestamptz>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    run_result_file (run_id, result_id) {
        run_id -> Uuid,
        result_id -> Uuid,
        uri -> Text,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    run_result_numeric (run_id, result_id) {
        run_id -> Uuid,
        result_id -> Uuid,
        value -> Float8,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    template (template_id) {
        template_id -> Uuid,
        pipeline_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        test_wdl -> Text,
        eval_wdl -> Text,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    template_result (template_id, result_id) {
        template_id -> Uuid,
        result_id -> Uuid,
        result_key -> Text,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::*;

    test (test_id) {
        test_id -> Uuid,
        template_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        test_input_defaults -> Nullable<Jsonb>,
        eval_input_defaults -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

allow_tables_to_appear_in_same_query!(
    pipeline,
    result,
    run,
    run_result_file,
    run_result_numeric,
    template,
    template_result,
    test,
);
