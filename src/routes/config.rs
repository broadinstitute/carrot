//! Defines REST API mappings for operations related to the carrot server's config
//!
//! Contains mappings for retrieving config information from the carrot server that may be relevant
//! to the user in some way (e.g. the cromwell server address, so they can interact with its API for
//! interrogating failed workflows)

use actix_web::{HttpResponse, web};
use actix_web::http::StatusCode;
use crate::config::Config;

/// Handles requests to /config/cromwell to retrieve the address of the cromwell server this carrot
/// server uses
fn get_cromwell_server_address(config: web::Data<Config>) -> HttpResponse {
    // Get the address from the config
    let mut cromwell_address: String = config.cromwell().address().to_string();
    // If it's a localhost address,
    return HttpResponse::Ok().body(config.cromwell().address());
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/config/cromwell")
            .route(web::get().to(get_cromwell_server_address))
    );
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http, test};
    use crate::config::Config;
    use crate::unit_test_util::load_default_config;
    use super::init_routes;

    #[actix_rt::test]
    async fn get_cromwell_server_address_success() {
        // Get a test config
        let test_config: Config = load_default_config();
        // Get the cromwell address so we can check against it later
        let true_address: String = test_config.cromwell().address().to_string();
        // Initialize test service with routes from super module
        let mut app = test::init_service(App::new().data(test_config).configure(init_routes)).await;
        // Make the API call
        let req = test::TestRequest::get()
            .uri("/config/cromwell")
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        // Make sure we got a good status code
        assert_eq!(resp.status(), http::StatusCode::OK);
        // Verify the address was returned
        let result = test::read_body(resp).await;
        let address = String::from_utf8(result.to_vec()).unwrap();
        assert_eq!(address, true_address);
    }

}