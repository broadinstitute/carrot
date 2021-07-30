//! Contains functions for checking if various features are enabled in the config and returning
//! http error responses if they are not

use crate::config::GCloudConfig;
use crate::routes::error_handling::ErrorBody;
use actix_web::HttpResponse;

/// Returns an http response with a message explaining that reporting is disabled
pub fn reporting_disabled_mapping() -> HttpResponse {
    HttpResponse::UnprocessableEntity().json(
        ErrorBody{
            title: "Reporting disabled".to_string(),
            status: 422,
            detail: "You are trying to access a reporting-related endpoint, but the reporting feature is disabled for this CARROT server".to_string(),
        }
    )
}

/// Returns an http response with a message explaining that software building is disabled
pub fn software_building_disabled_mapping() -> HttpResponse {
    HttpResponse::UnprocessableEntity().json(
        ErrorBody{
            title: "Software building disabled".to_string(),
            status: 422,
            detail: "You are trying to access a software-related endpoint, but the software building feature is disabled for this CARROT server".to_string(),
        }
    )
}

/// If using gs uris for wdls is enabled, returns Ok(()), otherwise returns an error containing an
/// HttpResponse explaining that using gs uris for wdls is disabled
pub fn is_gs_uris_for_wdls_enabled(
    gcloud_config: Option<&GCloudConfig>,
) -> Result<(), HttpResponse> {
    if let Some(gcloud_config) = gcloud_config {
        if gcloud_config.enable_gs_uris_for_wdl() {
            return Ok(());
        }
    }
    Err(
        HttpResponse::UnprocessableEntity().json(
            ErrorBody{
                title: "Using GS URIs for WDLs is disabled".to_string(),
                status: 422,
                detail: "You are trying to use a wdl accessed via a GS URI, but this feature is disabled for this CARROT server".to_string(),
            }
        )
    )
}

#[cfg(test)]
mod tests {
    use crate::config::GCloudConfig;
    use crate::routes::disabled_features::is_gs_uris_for_wdls_enabled;

    #[test]
    fn is_gs_uris_for_wdls_enabled_true() {
        let test_config: Option<GCloudConfig> = Some(GCloudConfig::new(String::from("test"), true));
        match is_gs_uris_for_wdls_enabled(test_config.as_ref()) {
            Ok(_) => {}
            Err(_) => panic!(),
        }
    }

    #[test]
    fn is_gs_uris_for_wdls_enabled_false_with_config() {
        let test_config: Option<GCloudConfig> =
            Some(GCloudConfig::new(String::from("test"), false));
        match is_gs_uris_for_wdls_enabled(test_config.as_ref()) {
            Ok(_) => panic!(),
            Err(_) => {}
        }
    }

    #[test]
    fn is_gs_uris_for_wdls_enabled_false_without_config() {
        match is_gs_uris_for_wdls_enabled(None) {
            Ok(_) => panic!(),
            Err(_) => {}
        }
    }
}
