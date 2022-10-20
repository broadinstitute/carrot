//! Contains utility functions required by unit tests within the models module

use crate::config;
use crate::config::{
    Config, CromwellConfig, CustomImageBuildConfig, LocalWdlStorageConfig,
    PrivateGithubAccessConfig, WdlStorageConfig,
};
use crate::custom_sql_types::MachineTypeEnum;
use crate::db;
use crate::manager::test_runner::TestRunner;
use crate::models::software::{NewSoftware, SoftwareData};
use crate::requests::cromwell_requests::CromwellClient;
use crate::requests::test_resource_requests::TestResourceClient;
use crate::util::git_repos::GitRepoManager;
use actix_web::client::Client;
use diesel::pg::PgConnection;
use diesel::Connection;
use dotenv;
use std::env;
use std::fs::read_to_string;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;
use tempfile::{NamedTempFile, TempDir};

embed_migrations!("migrations");

lazy_static! {
    pub static ref WDL_TEMP_DIR: TempDir = TempDir::new().unwrap();
    pub static ref GIT_TEMP_DIR: TempDir = TempDir::new().unwrap();
    pub static ref REMOTE_REPO_TEMP_DIR: TempDir = TempDir::new().unwrap();
}

// For creating DB schema only once before tests run
static INIT: Once = Once::new();

pub fn initialize_db_schema(conn: &PgConnection) {
    INIT.call_once(|| {
        let migrations_result = embedded_migrations::run_with_output(conn, &mut std::io::stdout());
        if let Err(e) = migrations_result {
            panic!("Database schema migrations failed with error: {}", e);
        }
    });
}

pub fn get_test_db_connection() -> PgConnection {
    // Load config so we can get DB connection string
    let config = load_default_config();
    // Get the DB url
    let db_url = String::from(config.database().url());
    // Connect
    let conn = PgConnection::establish(&db_url).expect("Failed to connect to database");
    // Initialize schema if necessary
    initialize_db_schema(&conn);
    // Start a test transaction, so test changes will not be committed
    conn.begin_test_transaction()
        .expect("Failed to create test transaction");

    conn
}

pub fn get_test_db_pool() -> db::DbPool {
    // Load config so we can get DB connection string
    let config = load_default_config();
    // Get the DB url
    let db_url = String::from(config.database().url());
    // Connect
    let conn = db::get_pool(&db_url, 1);
    // Initialize schema if necessary
    initialize_db_schema(&conn.get().unwrap());
    // Start a test transaction, so test changes will not be committed
    conn.get()
        .unwrap()
        .begin_test_transaction()
        .expect("Failed to create test transaction");

    conn
}

pub fn get_test_test_runner_building_enabled() -> TestRunner {
    let config: Config = load_default_config();

    let cromwell_client: CromwellClient =
        CromwellClient::new(Client::default(), &mockito::server_url());
    let test_resource_client: TestResourceClient = TestResourceClient::new(Client::default(), None);

    let image_build_config: CustomImageBuildConfig =
        config.custom_image_build().unwrap().to_owned();

    let git_repo_manager: GitRepoManager = GitRepoManager::new(
        image_build_config
            .private_github_access()
            .map(PrivateGithubAccessConfig::to_owned),
        image_build_config.repo_cache_location().to_owned(),
    );

    TestRunner::new(
        cromwell_client,
        test_resource_client,
        Some(image_build_config.image_registry_host()),
        Some(git_repo_manager),
    )
}

pub fn get_test_test_runner_building_disabled() -> TestRunner {
    let cromwell_client: CromwellClient =
        CromwellClient::new(Client::default(), &mockito::server_url());
    let test_resource_client: TestResourceClient = TestResourceClient::new(Client::default(), None);

    TestRunner::new(cromwell_client, test_resource_client, None, None)
}

/// Creates a git repo in `REMOTE_REPO_TEMP_DIR` with one file and two commits.  The first commit is
/// tagged with 'first' and 'beginning'.  Returns a path to the repo and the two commit hashes
pub fn get_test_remote_github_repo() -> (PathBuf, String, String) {
    // Load script we'll run
    let script = read_to_string("testdata/util/git_repo/create_test_repo.sh").unwrap();
    // Run script for filling repo
    let output = Command::new("sh")
        .current_dir(&*REMOTE_REPO_TEMP_DIR.path())
        .arg("-c")
        .arg(script)
        .output()
        .unwrap();

    let mut commits: Vec<String> = String::from_utf8_lossy(&*output.stdout)
        .split_whitespace()
        .map(String::from)
        .collect();
    let second_commit = commits.pop().unwrap();
    let first_commit = commits.pop().unwrap();

    (
        REMOTE_REPO_TEMP_DIR.path().to_path_buf(),
        first_commit,
        second_commit,
    )
}

pub fn insert_test_software_with_repo(conn: &PgConnection, repo_url: &str) -> SoftwareData {
    let new_software = NewSoftware {
        name: String::from("TestSoftware"),
        description: Some(String::from("Kevin made this software for testing")),
        repository_url: String::from(repo_url),
        machine_type: Some(MachineTypeEnum::Standard),
        created_by: Some(String::from("Kevin@example.com")),
    };

    SoftwareData::create(conn, new_software).unwrap()
}

pub fn load_default_config() -> Config {
    // Set up the WDL temp dir, since we can't load that from the test env file
    let wdl_storage_config = init_wdl_temp_dir();
    // Make a cromwell config that uses the mockito url
    let cromwell_config = CromwellConfig::new(mockito::server_url());
    // Load rest of config from test config file
    let config_string = read_to_string("testdata/test_config.yml")
        .expect("Failed to load testdata/test_config.yml");
    let mut test_config: Config = serde_yaml::from_str(&config_string)
        .expect("Failed to parse provided config file as a valid CARROT config");
    test_config.set_wdl_storage(wdl_storage_config);
    test_config.set_cromwell(cromwell_config);
    if let Some(custom_image_build_config) = test_config.custom_image_build() {
        test_config.set_custom_image_build(Some(CustomImageBuildConfig::new(
            custom_image_build_config.image_registry_host().clone(),
            custom_image_build_config.private_github_access().cloned(),
            String::from(&*GIT_TEMP_DIR.path().to_str().unwrap()),
        )));
    }

    test_config
}

pub fn get_temp_file(contents: &str) -> NamedTempFile {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{}", contents).unwrap();
    temp_file
}

pub fn init_wdl_temp_dir() -> WdlStorageConfig {
    WdlStorageConfig::Local(LocalWdlStorageConfig::new(String::from(
        &*WDL_TEMP_DIR.path().to_str().unwrap(),
    )))
}
