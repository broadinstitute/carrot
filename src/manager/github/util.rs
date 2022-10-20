//! Defines functions used in multiple places within the manager.github module

use crate::manager::github::Error;
use crate::manager::test_runner::TestRunner;
use crate::models::run::RunData;
use diesel::PgConnection;
use serde_json::{json, Value};
use uuid::Uuid;

/// Builds parameters for a custom docker build using the input_keys, `software_name`, and `commit`
/// to start a run and starts the run.  Returns either the RunData for the started run or an error
/// if it fails
pub async fn start_run_from_request(
    conn: &PgConnection,
    test_runner: &TestRunner,
    test_id: Uuid,
    run_group_id: Option<Uuid>,
    test_input_key: Option<&str>,
    eval_input_key: Option<&str>,
    software_name: &str,
    commit: &str,
) -> Result<RunData, Error> {
    // Build test and eval input jsons from the request, if it has values for the keys
    let test_input = match test_input_key {
        Some(key) => Some(build_input_from_key_and_software_and_commit(
            key,
            software_name,
            commit,
        )),
        None => None,
    };
    let eval_input = match eval_input_key {
        Some(key) => Some(build_input_from_key_and_software_and_commit(
            key,
            software_name,
            commit,
        )),
        None => None,
    };
    // Start run
    Ok(test_runner
        .create_run(
            conn,
            &test_id.to_string(),
            run_group_id,
            None,
            test_input,
            None,
            eval_input,
            None,
            None,
        )
        .await?)
}

/// Returns a json object containing one key value pair, with `input_key` as the key, and the value
/// set to the CARROT docker build value format: image_build:software_name|commit
fn build_input_from_key_and_software_and_commit(
    input_key: &str,
    software_name: &str,
    commit: &str,
) -> Value {
    json!({ input_key: format!("image_build:{}|{}", software_name, commit) })
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{
        BuildStatusEnum, EntityTypeEnum, MachineTypeEnum, RunStatusEnum,
    };
    use crate::manager::github::GithubRunRequest;
    use crate::manager::test_runner::TestRunner;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run_group::RunGroupData;
    use crate::models::run_software_version::RunSoftwareVersionData;
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{SoftwareBuildData, SoftwareBuildQuery};
    use crate::models::software_version::{SoftwareVersionData, SoftwareVersionQuery};
    use crate::models::subscription::{NewSubscription, SubscriptionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::requests::test_resource_requests::TestResourceClient;
    use crate::unit_test_util::{
        get_test_db_connection, get_test_remote_github_repo, get_test_test_runner_building_enabled,
        insert_test_software_with_repo,
    };
    use actix_web::client::Client;
    use diesel::PgConnection;
    use serde_json::json;
    use uuid::Uuid;

    fn insert_test_pipeline(conn: &PgConnection) -> PipelineData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline")
    }

    fn insert_test_template_with_pipeline_id(conn: &PgConnection, id: Uuid) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: id,
            description: None,
            test_wdl: format!("{}/test_software_params", mockito::server_url()),
            test_wdl_dependencies: None,
            eval_wdl: format!("{}/eval_software_params", mockito::server_url()),
            eval_wdl_dependencies: None,
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_test_with_subscriptions_with_entities(
        conn: &PgConnection,
        email_base_name: &str,
    ) -> TestData {
        let pipeline = insert_test_pipeline(conn);
        let template = insert_test_template_with_pipeline_id(conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(conn, template.template_id);

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Pipeline,
            entity_id: pipeline.pipeline_id,
            email: String::from(format!("{}@example.com", email_base_name)),
        };

        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Template,
            entity_id: template.template_id,
            email: String::from(format!("{}@example.com", email_base_name)),
        };

        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Test,
            entity_id: test.test_id,
            email: String::from(format!("{}@example.com", email_base_name)),
        };
        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        test
    }

    fn insert_test_run_group(conn: &PgConnection) -> RunGroupData {
        RunGroupData::create(conn).expect("Failed inserting test run_group")
    }

    #[actix_rt::test]
    async fn test_start_run_from_request() {
        let conn = get_test_db_connection();
        let test_test =
            insert_test_test_with_subscriptions_with_entities(&conn, "test_start_run_from_request");
        let (test_repo, commit1, _) = get_test_remote_github_repo();
        let test_software = insert_test_software_with_repo(&conn, test_repo.to_str().unwrap());
        let test_run_group = insert_test_run_group(&conn);
        let test_test_runner = get_test_test_runner_building_enabled();

        let test_request = GithubRunRequest {
            test_name: test_test.name,
            test_input_key: Some(String::from("in_test_image")),
            eval_input_key: Some(String::from("in_eval_image")),
            software_name: test_software.name,
            commit: commit1.clone(),
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleKevin"),
        };

        let test_params =
            json!({ "in_test_image": format!("image_build:TestSoftware|{}", commit1) });
        let eval_params =
            json!({ "in_eval_image": format!("image_build:TestSoftware|{}", commit1) });

        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .expect(0)
            .create();

        // Define mappings for resource request responses
        let test_wdl_mock = mockito::mock("GET", "/test_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .expect(0)
            .create();
        let eval_wdl_mock = mockito::mock("GET", "/eval_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .expect(0)
            .create();

        let test_run = super::start_run_from_request(
            &conn,
            &test_test_runner,
            test_test.test_id,
            Some(test_run_group.run_group_id),
            test_request.test_input_key.as_deref(),
            test_request.eval_input_key.as_deref(),
            &test_request.software_name,
            &test_request.commit,
        )
        .await
        .unwrap();

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(test_run.test_id, test_test.test_id);
        assert_eq!(test_run.status, RunStatusEnum::Building);
        assert_eq!(test_run.test_input, test_params);
        assert_eq!(test_run.eval_input, eval_params);

        let software_version_q = SoftwareVersionQuery {
            software_version_id: None,
            software_id: Some(test_software.software_id),
            commit: Some(commit1),
            software_name: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let created_software_version =
            SoftwareVersionData::find(&conn, software_version_q).unwrap();
        assert_eq!(created_software_version.len(), 1);

        let software_build_q = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: Some(created_software_version[0].software_version_id),
            build_job_id: None,
            status: Some(BuildStatusEnum::Created),
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let created_software_build = SoftwareBuildData::find(&conn, software_build_q).unwrap();
        assert_eq!(created_software_build.len(), 1);

        let created_run_software_version =
            RunSoftwareVersionData::find_by_run_and_software_version(
                &conn,
                test_run.run_id,
                created_software_version[0].software_version_id,
            )
            .unwrap();
    }

    #[test]
    fn test_build_input_from_key_and_software_and_commit() {
        let input_key = "test_docker";
        let software_name = "test_software";
        let commit = "ca82a6dff817ec66f44342007202690a93763949";

        let expected_result = json!({
            "test_docker": "image_build:test_software|ca82a6dff817ec66f44342007202690a93763949"
        });

        let result =
            super::build_input_from_key_and_software_and_commit(input_key, software_name, commit);

        assert_eq!(result, expected_result);
    }
}
