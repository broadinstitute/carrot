//! Contains function(s) for converting internal resource locations into URIs for users to access
//! those resources

use crate::models::run::{RunData, RunWithResultsAndErrorsData};
use crate::models::template::TemplateData;
use crate::util::wdl_type::WdlType;
use uuid::Uuid;

/// Checks the values in `run` for `test_wdl`, `eval_wdl`, `test_wdl_dependencies`, and
/// `eval_wdl_dependencies` and fills URIs for the user to use to retrieve them (either keeping
/// them the same if they are gs://, http://, or https://; or replacing with the download REST
/// mapping based on the `host` value)
pub fn fill_uris_for_wdl_locations_run(host: &str, run: &mut RunData) {
    let (test_wdl, test_wdl_dependencies, eval_wdl, eval_wdl_dependencies) =
        get_uris_for_wdl_locations(
            host,
            "runs",
            run.run_id,
            &run.test_wdl,
            run.test_wdl_dependencies.as_deref(),
            &run.eval_wdl,
            run.eval_wdl_dependencies.as_deref(),
        );
    run.test_wdl = test_wdl;
    run.eval_wdl = eval_wdl;
    run.test_wdl_dependencies = test_wdl_dependencies;
    run.eval_wdl_dependencies = eval_wdl_dependencies;
}

/// Checks the values in `run` for `test_wdl`, `eval_wdl`, `test_wdl_dependencies`, and
/// `eval_wdl_dependencies` and fills URIs for the user to use to retrieve them (either keeping
/// them the same if they are gs://, http://, or https://; or replacing with the download REST
/// mapping based on the `host` value)
pub fn fill_uris_for_wdl_locations_run_with_results_and_errors(
    host: &str,
    run: &mut RunWithResultsAndErrorsData,
) {
    let (test_wdl, test_wdl_dependencies, eval_wdl, eval_wdl_dependencies) =
        get_uris_for_wdl_locations(
            host,
            "runs",
            run.run_id,
            &run.test_wdl,
            run.test_wdl_dependencies.as_deref(),
            &run.eval_wdl,
            run.eval_wdl_dependencies.as_deref(),
        );
    run.test_wdl = test_wdl;
    run.eval_wdl = eval_wdl;
    run.test_wdl_dependencies = test_wdl_dependencies;
    run.eval_wdl_dependencies = eval_wdl_dependencies;
}

/// Checks the values in `template` for `test_wdl`, `eval_wdl`, `test_wdl_dependencies`, and
/// `eval_wdl_dependencies` and fills URIs for the user to use to retrieve them (either keeping
/// them the same if they are gs://, http://, or https://; or replacing with the download REST
/// mapping based on the `host` value)
pub fn fill_uris_for_wdl_locations_template(host: &str, template: &mut TemplateData) {
    let (test_wdl, test_wdl_dependencies, eval_wdl, eval_wdl_dependencies) =
        get_uris_for_wdl_locations(
            host,
            "templates",
            template.template_id,
            &template.test_wdl,
            template.test_wdl_dependencies.as_deref(),
            &template.eval_wdl,
            template.eval_wdl_dependencies.as_deref(),
        );
    template.test_wdl = test_wdl;
    template.eval_wdl = eval_wdl;
    template.test_wdl_dependencies = test_wdl_dependencies;
    template.eval_wdl_dependencies = eval_wdl_dependencies;
}

/// Checks the values for `test_wdl`, `eval_wdl`, `test_wdl_dependencies`, and
/// `eval_wdl_dependencies` and returns URIs for the user to use to retrieve them (either keeping
/// them the same if they are gs://, http://, or https://; or replacing with the download REST
/// mapping based on the `host` value)
fn get_uris_for_wdl_locations(
    host: &str,
    entity: &str,
    entity_id: Uuid,
    test_wdl: &str,
    test_wdl_dependencies: Option<&str>,
    eval_wdl: &str,
    eval_wdl_dependencies: Option<&str>,
) -> (String, Option<String>, String, Option<String>) {
    let converted_test_wdl =
        get_uri_for_wdl_or_deps_location(host, test_wdl, entity, entity_id, true, WdlType::Test);
    let converted_eval_wdl =
        get_uri_for_wdl_or_deps_location(host, eval_wdl, entity, entity_id, true, WdlType::Eval);
    let converted_test_wdl_dependencies = match test_wdl_dependencies {
        Some(dep_location) => Some(get_uri_for_wdl_or_deps_location(
            host,
            dep_location,
            entity,
            entity_id,
            false,
            WdlType::Test,
        )),
        None => None,
    };
    let converted_eval_wdl_dependencies = match eval_wdl_dependencies {
        Some(dep_location) => Some(get_uri_for_wdl_or_deps_location(
            host,
            dep_location,
            entity,
            entity_id,
            false,
            WdlType::Eval,
        )),
        None => None,
    };

    (
        converted_test_wdl,
        converted_test_wdl_dependencies,
        converted_eval_wdl,
        converted_eval_wdl_dependencies,
    )
}

/// Returns a URI that the user can use to retrieve the wdl (if `is_wdl`) or dependency zip at
/// `location`
/// For gs: and http/https: locations, it just returns the location.  For local file locations,
/// returns a REST URI for accessing it, using `entity` and `entity_id` for the entity and id for
/// the uri.
fn get_uri_for_wdl_or_deps_location(
    host: &str,
    location: &str,
    entity: &str,
    entity_id: Uuid,
    is_wdl: bool,
    wdl_type: WdlType,
) -> String {
    // If the location starts with gs://, http://, or https://, we'll just return it, since the
    // user can use that to retrieve the wdl
    if location.starts_with("gs://")
        || location.starts_with("http://")
        || location.starts_with("https://")
    {
        return String::from(location);
    }
    // Otherwise, we assume it's a file, so we build the REST mapping the user can use to access it
    format!(
        "{}/api/v1/{}/{}/{}_wdl{}",
        host,
        entity,
        entity_id,
        wdl_type,
        if is_wdl { "" } else { "_dependencies" }
    )
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn get_uri_for_wdl_or_deps_location_local_wdl() {
        let uri = get_uri_for_wdl_or_deps_location(
            "example.com",
            "~/.carrot/wdl/ca6c60fd-7fb6-4259-99ee-9e59f834741a/test.wdl",
            "runs",
            Uuid::parse_str("fac03462-7a29-45e0-8479-8e1423d62678").unwrap(),
            true,
            WdlType::Test,
        );
        assert_eq!(
            uri,
            "example.com/api/v1/runs/fac03462-7a29-45e0-8479-8e1423d62678/test_wdl"
        );
    }

    #[test]
    fn get_uri_for_wdl_or_deps_location_local_deps() {
        let uri = get_uri_for_wdl_or_deps_location(
            "example.com",
            "~/.carrot/wdl/ca6c60fd-7fb6-4259-99ee-9e59f834741a/test.zip",
            "runs",
            Uuid::parse_str("fac03462-7a29-45e0-8479-8e1423d62678").unwrap(),
            false,
            WdlType::Test,
        );
        assert_eq!(
            uri,
            "example.com/api/v1/runs/fac03462-7a29-45e0-8479-8e1423d62678/test_wdl_dependencies"
        );
    }

    #[test]
    fn get_uri_for_wdl_or_deps_location_gcs() {
        let uri = get_uri_for_wdl_or_deps_location(
            "example.com",
            "gs://example/path/to/wdl",
            "runs",
            Uuid::parse_str("fac03462-7a29-45e0-8479-8e1423d62678").unwrap(),
            true,
            WdlType::Test,
        );
        assert_eq!(uri, "gs://example/path/to/wdl");
    }
}
