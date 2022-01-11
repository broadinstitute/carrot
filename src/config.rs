//! Contains and loads lazy_static references for all of the configuration variables that can be
//! specified in environment variables and/or a config file.  Those variables should be accessed
//! from here instead of loaded again elsewhere

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top level config struct that holds other area-specific configs
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    logging: LoggingConfig,
    api: ApiConfig,
    database: DatabaseConfig,
    cromwell: CromwellConfig,
    // We don't need to specify a function for these defaults because we implemented Default for
    // the structs
    #[serde(default)]
    status_manager: StatusManagerConfig,
    #[serde(default)]
    wdl_storage: WdlStorageConfig,
    email: Option<EmailConfig>,
    gcloud: Option<GCloudConfig>,
    github: Option<GithubConfig>,
    custom_image_build: Option<CustomImageBuildConfig>,
    validation: ValidationConfig,
    reporting: Option<ReportingConfig>,
}

impl Config {
    /// Create a new instance of Config by specifying sub configs.
    ///
    /// # Panics
    /// Panics if attempting to create a Config that has:
    /// 1. A value for `github` and None for `custom_image_build`,
    /// 2. A value for `custom_image_builds` and None for `gcloud`, or
    /// 3. A value for `reporting` and None for `gcloud`
    pub fn new(
        logging: LoggingConfig,
        api: ApiConfig,
        database: DatabaseConfig,
        cromwell: CromwellConfig,
        status_manager: StatusManagerConfig,
        wdl_storage: WdlStorageConfig,
        email: Option<EmailConfig>,
        gcloud: Option<GCloudConfig>,
        github: Option<GithubConfig>,
        custom_image_build: Option<CustomImageBuildConfig>,
        validation: ValidationConfig,
        reporting: Option<ReportingConfig>,
    ) -> Self {
        // Create new config from params
        let new_config = Config {
            logging,
            api,
            database,
            cromwell,
            status_manager,
            wdl_storage,
            email,
            gcloud,
            github,
            custom_image_build,
            validation,
            reporting,
        };
        // Validate it
        new_config.validate();
        // Return if there are not problems
        new_config
    }
    /// Validates config to make sure subconfigs that are dependent on other subconfigs have those
    /// dependencies present.  Panics if not
    ///
    /// # Panics
    /// Panics if attempting to create a Config that has:
    /// 1. A value for `github` and None for `custom_image_build`,
    /// 2. A value for `custom_image_builds` and None for `gcloud`, or
    /// 3. A value for `reporting` and None for `gcloud`
    pub fn validate(&self) {
        if self.github.is_some() && self.custom_image_build.is_none() {
            panic!("In order to enable Github integration, it is necessary to specify a configuration for \"custom_image_build\"");
        }
        if self.custom_image_build.is_some() && self.gcloud.is_none() {
            panic!("In order to enable custom image building, it is necessary to specify a configuration for \"gcloud\"");
        }
        if self.reporting.is_some() && self.gcloud.is_none() {
            panic!("In order to enable reporting, it is necessary to specify a configuration for \"gcloud\"");
        }
    }
    pub fn logging(&self) -> &LoggingConfig {
        &self.logging
    }
    pub fn api(&self) -> &ApiConfig {
        &self.api
    }
    pub fn database(&self) -> &DatabaseConfig {
        &self.database
    }
    pub fn cromwell(&self) -> &CromwellConfig {
        &self.cromwell
    }
    pub fn status_manager(&self) -> &StatusManagerConfig {
        &self.status_manager
    }
    pub fn wdl_storage(&self) -> &WdlStorageConfig {
        &self.wdl_storage
    }
    pub fn email(&self) -> Option<&EmailConfig> {
        self.email.as_ref()
    }
    pub fn gcloud(&self) -> Option<&GCloudConfig> {
        self.gcloud.as_ref()
    }
    pub fn github(&self) -> Option<&GithubConfig> {
        self.github.as_ref()
    }
    pub fn custom_image_build(&self) -> Option<&CustomImageBuildConfig> {
        self.custom_image_build.as_ref()
    }
    pub fn validation(&self) -> &ValidationConfig {
        &self.validation
    }
    pub fn reporting(&self) -> Option<&ReportingConfig> {
        self.reporting.as_ref()
    }

    // For tests, we want these to be mutable so we can change them if we need to
    #[cfg(test)]
    pub fn set_logging(&mut self, logging: LoggingConfig) {
        self.logging = logging;
    }
    #[cfg(test)]
    pub fn set_api(&mut self, api: ApiConfig) {
        self.api = api;
    }
    #[cfg(test)]
    pub fn set_database(&mut self, database: DatabaseConfig) {
        self.database = database;
    }
    #[cfg(test)]
    pub fn set_cromwell(&mut self, cromwell: CromwellConfig) {
        self.cromwell = cromwell;
    }
    #[cfg(test)]
    pub fn set_status_manager(&mut self, status_manager: StatusManagerConfig) {
        self.status_manager = status_manager;
    }
    #[cfg(test)]
    pub fn set_wdl_storage(&mut self, wdl_storage: WdlStorageConfig) {
        self.wdl_storage = wdl_storage;
    }
    #[cfg(test)]
    pub fn set_email(&mut self, email: Option<EmailConfig>) {
        self.email = email;
    }
    #[cfg(test)]
    pub fn set_gcloud(&mut self, gcloud: Option<GCloudConfig>) {
        self.gcloud = gcloud;
    }
    #[cfg(test)]
    pub fn set_github(&mut self, github: Option<GithubConfig>) {
        self.github = github;
    }
    #[cfg(test)]
    pub fn set_custom_image_build(&mut self, custom_image_build: Option<CustomImageBuildConfig>) {
        self.custom_image_build = custom_image_build;
    }
    #[cfg(test)]
    pub fn set_validation(&mut self, validation: ValidationConfig) {
        self.validation = validation;
    }
    #[cfg(test)]
    pub fn set_reporting(&mut self, reporting: Option<ReportingConfig>) {
        self.reporting = reporting;
    }
}

/// Config for setting up logging
#[derive(Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "logging_level_default")]
    level: log::Level,
    #[serde(default)]
    modules: HashMap<String, log::Level>,
}

// Function for providing the default value
fn logging_level_default() -> log::Level {
    log::Level::Info
}

impl LoggingConfig {
    pub fn new(level: log::Level, modules: HashMap<String, log::Level>) -> Self {
        LoggingConfig { level, modules }
    }
    pub fn level(&self) -> &log::Level {
        &self.level
    }
    pub fn modules(&self) -> &HashMap<String, log::Level> {
        &self.modules
    }
}

/// Config for setting up the REST API
#[derive(Serialize, Deserialize, Clone)]
pub struct ApiConfig {
    /// Host address for the application
    host: String,
    /// Host port for the application
    port: String,
}

impl ApiConfig {
    pub fn new(host: String, port: String) -> Self {
        ApiConfig { host, port }
    }
    pub fn host(&self) -> &String {
        &self.host
    }
    pub fn port(&self) -> &String {
        &self.port
    }
}

/// Config for connecting to the DB
#[derive(Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    /// Connection URL for the database
    url: String,
    /// Number of threads to use when connecting to the database
    threads: u32,
}

impl DatabaseConfig {
    pub fn new(url: String, threads: u32) -> Self {
        DatabaseConfig { url, threads }
    }
    pub fn url(&self) -> &String {
        &self.url
    }
    pub fn threads(&self) -> u32 {
        self.threads
    }
}

/// Config for dispatching jobs to Cromwell
#[derive(Serialize, Deserialize, Clone)]
pub struct CromwellConfig {
    /// The address for the cromwell server that will be used to run tests
    address: String,
}

impl CromwellConfig {
    pub fn new(address: String) -> Self {
        CromwellConfig { address }
    }
    pub fn address(&self) -> &String {
        &self.address
    }
}

/// Config for the status manager
#[derive(Serialize, Deserialize, Clone)]
pub struct StatusManagerConfig {
    /// Time to wait between status check queries, or default to 5 minutes
    #[serde(default = "status_check_wait_time_in_secs_default")]
    status_check_wait_time_in_secs: u64,
    /// Number of consecutive status check failures to allow before panicking, or default to 5
    #[serde(default = "allowed_consecutive_status_check_failures_default")]
    allowed_consecutive_status_check_failures: u32,
}

// Functions for providing the default values
fn status_check_wait_time_in_secs_default() -> u64 {
    300
}
fn allowed_consecutive_status_check_failures_default() -> u32 {
    5
}

impl Default for StatusManagerConfig {
    fn default() -> Self {
        StatusManagerConfig {
            status_check_wait_time_in_secs: status_check_wait_time_in_secs_default(),
            allowed_consecutive_status_check_failures:
                allowed_consecutive_status_check_failures_default(),
        }
    }
}

impl StatusManagerConfig {
    pub fn new(
        status_check_wait_time_in_secs: u64,
        allowed_consecutive_status_check_failures: u32,
    ) -> Self {
        StatusManagerConfig {
            status_check_wait_time_in_secs,
            allowed_consecutive_status_check_failures,
        }
    }
    pub fn status_check_wait_time_in_secs(&self) -> u64 {
        self.status_check_wait_time_in_secs
    }
    pub fn allowed_consecutive_status_check_failures(&self) -> u32 {
        self.allowed_consecutive_status_check_failures
    }
}

/// Config for sending email notifications
#[derive(Serialize, Deserialize, Clone)]
pub enum EmailConfig {
    /// Mode for sending emails via a mail server
    #[serde(rename = "server")]
    Server(EmailServerConfig),
    /// Mode for sending emails via the Unix sendmail utility
    #[serde(rename = "sendmail")]
    Sendmail(EmailSendmailConfig),
}

impl EmailConfig {
    pub fn is_server(&self) -> bool {
        match self {
            EmailConfig::Server(_) => true,
            _ => false,
        }
    }
    pub fn as_server(&self) -> Option<&EmailServerConfig> {
        match self {
            EmailConfig::Server(s) => Some(s),
            _ => None,
        }
    }
    pub fn is_sendmail(&self) -> bool {
        match self {
            EmailConfig::Sendmail(_) => true,
            _ => false,
        }
    }
    pub fn as_sendmail(&self) -> Option<&EmailSendmailConfig> {
        match self {
            EmailConfig::Sendmail(s) => Some(s),
            _ => None,
        }
    }
}

/// Config for sending emails in server mode
#[derive(Serialize, Deserialize, Clone)]
pub struct EmailServerConfig {
    /// Value to use in 'from' field in email notifications
    from: String,
    /// Domain for email server for notifications
    domain: String,
    /// Email server username if it exists
    username: Option<String>,
    /// Email server password if it exists
    password: Option<String>,
}

impl EmailServerConfig {
    pub fn new(
        from: String,
        domain: String,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        EmailServerConfig {
            from,
            domain,
            username,
            password,
        }
    }
    pub fn from(&self) -> &String {
        &self.from
    }
    pub fn domain(&self) -> &String {
        &self.domain
    }
    pub fn username(&self) -> Option<&String> {
        self.username.as_ref()
    }
    pub fn password(&self) -> Option<&String> {
        self.password.as_ref()
    }
}

/// Config for sending emails in sendmail mode
#[derive(Serialize, Deserialize, Clone)]
pub struct EmailSendmailConfig {
    /// Value to use in 'from' field in email notifications
    from: String,
}

impl EmailSendmailConfig {
    pub fn new(from: String) -> Self {
        EmailSendmailConfig { from }
    }
    pub fn from(&self) -> &String {
        &self.from
    }
}

/// Config for connecting to and interacting with google cloud
#[derive(Serialize, Deserialize, Clone)]
pub struct GCloudConfig {
    /// The location of the key file for the service account to use with GCloud services
    gcloud_sa_key_file: String,
    /// For enabling retrieving WDLs via GS URIs
    enable_gs_uris_for_wdl: bool,
}

impl GCloudConfig {
    pub fn new(gcloud_sa_key_file: String, enable_gs_uris_for_wdl: bool) -> Self {
        GCloudConfig {
            gcloud_sa_key_file,
            enable_gs_uris_for_wdl,
        }
    }
    pub fn gcloud_sa_key_file(&self) -> &String {
        &self.gcloud_sa_key_file
    }
    pub fn enable_gs_uris_for_wdl(&self) -> bool {
        self.enable_gs_uris_for_wdl
    }
}

/// Config for where WDLs should be stored
#[derive(Serialize, Deserialize, Clone)]
pub struct WdlStorageConfig {
    /// Local directory in which to store WDLs
    #[serde(default = "wdl_directory_default")]
    wdl_directory: String,
}

// Function for providing the default value
fn wdl_directory_default() -> String {
    let mut current_dir =
        std::env::current_dir().expect("Failed to get current directory for wdl directory default");
    current_dir.push("carrot");
    current_dir.push("wdl");
    current_dir
        .to_str()
        .expect("Failed to convert wdl directory path to string")
        .to_string()
}

// Defining a default value for WdlStorageConfig so the user doesn't have to explicitly specify it
impl Default for WdlStorageConfig {
    fn default() -> Self {
        WdlStorageConfig {
            wdl_directory: wdl_directory_default(),
        }
    }
}

impl WdlStorageConfig {
    pub fn new(wdl_directory: String) -> Self {
        WdlStorageConfig { wdl_directory }
    }
    pub fn wdl_directory(&self) -> &String {
        &self.wdl_directory
    }
}

/// Configuration for github integration functionality
#[derive(Serialize, Deserialize, Clone)]
pub struct GithubConfig {
    /// User ID for authentication with github api
    client_id: String,
    /// User token for authentication with github api
    client_token: String,
    /// The Google Cloud Pubsub subscription name from which messages from github to trigger test
    /// runs will be retrieved
    pubsub_subscription_name: String,
    /// The maximum number of messages to retrieve from the pubsub subscription at once
    #[serde(default = "pubsub_max_messages_per_default")]
    pubsub_max_messages_per: i32,
    /// The number of time, in seconds, to wait between checks of the pubsub subscription
    #[serde(default = "pubsub_wait_time_in_secs_default")]
    pubsub_wait_time_in_secs: u64,
}

// Functions for providing the default values
fn pubsub_max_messages_per_default() -> i32 {
    20
}
fn pubsub_wait_time_in_secs_default() -> u64 {
    60
}

impl GithubConfig {
    pub fn new(
        client_id: String,
        client_token: String,
        pubsub_subscription_name: String,
        pubsub_max_messages_per: i32,
        pubsub_wait_time_in_secs: u64,
    ) -> Self {
        GithubConfig {
            client_id,
            client_token,
            pubsub_subscription_name,
            pubsub_max_messages_per,
            pubsub_wait_time_in_secs,
        }
    }
    pub fn client_id(&self) -> &String {
        &self.client_id
    }
    pub fn client_token(&self) -> &String {
        &self.client_token
    }
    pub fn pubsub_subscription_name(&self) -> &String {
        &self.pubsub_subscription_name
    }
    pub fn pubsub_max_messages_per(&self) -> i32 {
        self.pubsub_max_messages_per
    }
    pub fn pubsub_wait_time_in_secs(&self) -> u64 {
        self.pubsub_wait_time_in_secs
    }
}

/// Config for building custom docker images from git repos
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomImageBuildConfig {
    /// The host address for the docker image registry where we'll store custom docker images
    image_registry_host: String,
    /// Config for accessing private github repos, if wanted
    private_github_access: Option<PrivateGithubAccessConfig>,
}

impl CustomImageBuildConfig {
    pub fn new(
        image_registry_host: String,
        private_github_access: Option<PrivateGithubAccessConfig>,
    ) -> Self {
        CustomImageBuildConfig {
            image_registry_host,
            private_github_access,
        }
    }
    pub fn image_registry_host(&self) -> &String {
        &self.image_registry_host
    }
    pub fn private_github_access(&self) -> Option<&PrivateGithubAccessConfig> {
        self.private_github_access.as_ref()
    }
}

/// Config for accessing private github repos
#[derive(Serialize, Deserialize, Clone)]
pub struct PrivateGithubAccessConfig {
    /// The github username for the user to use to access private repositories
    client_id: String,
    /// The github user token for the user to use to access private repositories
    client_token: String,
    /// The GCS URL of a google kms encrypted file containing the password for the account specified by `client_id`
    client_pass_uri: String,
    /// The GCloud KMS keyring for decrypting `client_pass_uri`
    kms_keyring: String,
    /// The GCloud KMS key for decrypting `client_pass_uri`
    kms_key: String,
}

impl PrivateGithubAccessConfig {
    pub fn new(
        client_id: String,
        client_token: String,
        client_pass_uri: String,
        kms_keyring: String,
        kms_key: String,
    ) -> Self {
        PrivateGithubAccessConfig {
            client_id,
            client_token,
            client_pass_uri,
            kms_keyring,
            kms_key,
        }
    }
    pub fn client_id(&self) -> &String {
        &self.client_id
    }
    pub fn client_token(&self) -> &String {
        &self.client_token
    }
    pub fn client_pass_uri(&self) -> &String {
        &self.client_pass_uri
    }
    pub fn kms_keyring(&self) -> &String {
        &self.kms_keyring
    }
    pub fn kms_key(&self) -> &String {
        &self.kms_key
    }
}

/// Config for validating parts of a test
#[derive(Serialize, Deserialize, Clone)]
pub struct ValidationConfig {
    /// The location of the womtool jar to use to validate WDLs
    womtool_location: String,
}

impl ValidationConfig {
    pub fn new(womtool_location: String) -> Self {
        ValidationConfig { womtool_location }
    }
    pub fn womtool_location(&self) -> &String {
        &self.womtool_location
    }
}

/// Config for reporting functionality
#[derive(Serialize, Deserialize, Clone)]
pub struct ReportingConfig {
    /// GCS directory where we'll put generated report files (in the form bucket-name/my/report/directory)
    report_location: String,
    /// Docker repo location of the docker image that will be used to run the report generation WDLs
    report_docker_location: String,
}

impl ReportingConfig {
    pub fn new(report_location: String, report_docker_location: String) -> Self {
        ReportingConfig {
            report_location,
            report_docker_location,
        }
    }
    pub fn report_location(&self) -> &String {
        &self.report_location
    }
    pub fn report_docker_location(&self) -> &String {
        &self.report_docker_location
    }
}
