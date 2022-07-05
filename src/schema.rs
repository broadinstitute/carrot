//! Defines the diesel schema for interfacing with the DB
//!
//! Uses diesel's table! macro to define the mappings to the tables in the DB.  The macro
//! generates the crate::schema::[table_name]::dsl module for each table, which allows performing
//! operations on the data in the tables

table! {
    use diesel::sql_types::*;

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
    use crate::custom_sql_types::Result_type_enum;

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
    use crate::custom_sql_types::Run_status_enum;

    run (run_id) {
        run_id -> Uuid,
        test_id -> Uuid,
        name -> Text,
        status -> Run_status_enum,
        test_input -> Jsonb,
        test_options -> Nullable<Jsonb>,
        eval_input -> Jsonb,
        eval_options -> Nullable<Jsonb>,
        test_cromwell_job_id -> Nullable<Text>,
        eval_cromwell_job_id -> Nullable<Text>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
        finished_at -> Nullable<Timestamptz>,
    }
}

table! {
    use diesel::sql_types::*;

    run_result (run_id, result_id) {
        run_id -> Uuid,
        result_id -> Uuid,
        value -> Text,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;

    template (template_id) {
        template_id -> Uuid,
        pipeline_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        test_wdl -> Text,
        test_wdl_dependencies -> Nullable<Text>,
        eval_wdl -> Text,
        eval_wdl_dependencies -> Nullable<Text>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;

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

    test (test_id) {
        test_id -> Uuid,
        template_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        test_input_defaults -> Nullable<Jsonb>,
        test_option_defaults -> Nullable<Jsonb>,
        eval_input_defaults -> Nullable<Jsonb>,
        eval_option_defaults -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::Run_status_enum;

    run_with_results_and_errors (run_id) {
        run_id -> Uuid,
        test_id -> Uuid,
        name -> Text,
        status -> Run_status_enum,
        test_input -> Jsonb,
        test_options -> Nullable<Jsonb>,
        eval_input -> Jsonb,
        eval_options -> Nullable<Jsonb>,
        test_cromwell_job_id -> Nullable<Text>,
        eval_cromwell_job_id -> Nullable<Text>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
        finished_at -> Nullable<Timestamptz>,
        results -> Nullable<Jsonb>,
        errors -> Nullable<Jsonb>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::Entity_type_enum;

    subscription(subscription_id) {
        subscription_id -> Uuid,
        entity_type -> Entity_type_enum,
        entity_id -> Uuid,
        email -> Text,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::Machine_type_enum;

    software(software_id) {
        software_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        repository_url -> Text,
        machine_type -> Machine_type_enum,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;

    software_version(software_version_id) {
        software_version_id -> Uuid,
        software_id -> Uuid,
        commit -> Text,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;

    run_software_version(run_id, software_version_id) {
        run_id -> Uuid,
        software_version_id -> Uuid,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::Build_status_enum;

    software_build(software_build_id) {
        software_build_id -> Uuid,
        software_version_id -> Uuid,
        build_job_id -> Nullable<Text>,
        status -> Build_status_enum,
        image_url -> Nullable<Text>,
        created_at -> Timestamptz,
        finished_at -> Nullable<Timestamptz>,
    }
}

table! {
    use diesel::sql_types::*;

    run_is_from_github(run_id) {
        run_id -> Uuid,
        owner -> Text,
        repo -> Text,
        issue_number -> Integer,
        author -> Text,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;

    report(report_id) {
        report_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        notebook -> Jsonb,
        config -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;

    template_report(template_id, report_id) {
        template_id -> Uuid,
        report_id -> Uuid,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_sql_types::Report_status_enum;

    run_report(run_id, report_id) {
        run_id -> Uuid,
        report_id -> Uuid,
        status -> Report_status_enum,
        cromwell_job_id -> Nullable<Text>,
        results -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        created_by -> Nullable<Text>,
        finished_at -> Nullable<Timestamptz>,
    }
}

table! {
    use diesel::sql_types::*;

    wdl_hash(location, hash) {
        location -> Text,
        hash -> Binary,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;

    run_error(run_error_id) {
        run_error_id -> Uuid,
        run_id -> Uuid,
        error -> Text,
        created_at -> Timestamptz,
    }
}

joinable!(test -> template(template_id));
joinable!(software_version -> software(software_id));

allow_tables_to_appear_in_same_query!(
    pipeline,
    result,
    run,
    run_result,
    template,
    template_result,
    test,
    run_with_results_and_errors,
    software,
    software_version,
    software_build,
    run_software_version,
    subscription,
    run_is_from_github,
    report,
    template_report,
    run_report,
    run_error,
);
