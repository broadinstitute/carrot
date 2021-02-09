//! Contains and loads lazy_static references for all of the configuration variables that can be
//! specified in environment variables and/or a config file.  Those variables should be accessed
//! from here instead of loaded again elsewhere

use crate::notifications::emailer::EmailMode;
use log::info;
use std::env;
use std::str::FromStr;

lazy_static! {

    // API Config
    /// Host address for the application
    pub static ref HOST: String = env::var("HOST").expect("HOST environment variable not set");
    /// Host port for the application
    pub static ref PORT: String = env::var("PORT").expect("PORT environment variable not set");

    // Database
    /// Connection URL for the database
    pub static ref DATABASE_URL: String =
        env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    /// Number of threads to use when connecting to the database
    pub static ref DB_THREADS: u32 =
        env::var("DB_THREADS").expect("DB_THREADS environment variable not set")
            .parse()
            .expect("DB_THREADS environment variable must be an integer");

    // Config
    /// The address for the cromwell server that will be used to run tests
    pub static ref CROMWELL_ADDRESS: String  = {
        env::var("CROMWELL_ADDRESS").expect("CROMWELL_ADDRESS environment variable not set")
    };

    // Status-checking config
    /// Time to wait between status check queries, or default to 5 minutes
    pub static ref STATUS_CHECK_WAIT_TIME_IN_SECS: u64 = {
        match env::var("STATUS_CHECK_WAIT_TIME_IN_SECS") {
            Ok(s) => s.parse::<u64>().unwrap(),
            Err(_) => {
                info!("No status check wait time specified.  Defaulting to 5 minutes");
                300
            }
        }
    };
    /// Number of consecutive status check failures to allow before panicking, or default to 5
    pub static ref ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES: u32 = {
        match env::var("ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES") {
            Ok(s) => s.parse::<u32>().unwrap(),
            Err(_) => {
                info!("No allowed consecutive status check failures specified.  Defaulting to 5 failures");
                5
            }
        }
    };

    // Emailer
    /// Mode we'll use for sending mail
    pub static ref EMAIL_MODE: EmailMode = EmailMode::from_str(&env::var("EMAIL_MODE")
        .expect("EMAIL_MODE environment variable not set"))
        .expect("EMAIL_MODE must be one of three values: SERVER, SENDMAIL, or NONE");
    /// Value to use in 'from' field in email notifications (if mode isn't None)
    pub static ref EMAIL_FROM: Option<String> = {
        match *EMAIL_MODE {
            EmailMode::None => None,
            _ => Some(env::var("EMAIL_FROM").expect("EMAIL_FROM environment variable not set"))
        }
    };
    /// Domain for email server for notifications
    pub static ref EMAIL_DOMAIN: Option<String> = {
        match *EMAIL_MODE {
            EmailMode::Server => Some(env::var("EMAIL_DOMAIN").expect("EMAIL_DOMAIN environment variable not set")),
            _ => None,
        }
    };
    /// Email server username if it exists
    pub static ref EMAIL_USERNAME: Option<String> = {
        match env::var("EMAIL_USERNAME") {
            Ok(s) => Some(s),
            Err(_) =>  {
                info!("No value specified for EMAIL_USERNAME");
                None
            }
        }
    };
    /// Email server password if it exists
    pub static ref EMAIL_PASSWORD: Option<String> = {
        match env::var("EMAIL_PASSWORD") {
            Ok(s) => Some(s),
            Err(_) =>  {
                info!("No value specified for EMAIL_PASSWORD");
                None
            }
        }
    };

    // GCloud
    /// The location of the key file for the service account to use with GCloud services
    pub static ref GCLOUD_SA_KEY_FILE: String =
        env::var("GCLOUD_SA_KEY_FILE").expect("GCLOUD_SA_KEY_FILE environment variable not set");

    // GITHUB
    /// User ID for authentication with github api
    pub static ref GITHUB_CLIENT_ID: String = env::var("GITHUB_CLIENT_ID")
        .expect("GITHUB_CLIENT_ID environment variable not set");
    /// User token for authentication with github api
    pub static ref GITHUB_CLIENT_TOKEN: String = env::var("GITHUB_CLIENT_TOKEN")
        .expect("GITHUB_CLIENT_TOKEN environment variable not set");
    /// If true, enables triggering carrot test runs from github
    pub static ref ENABLE_GITHUB_REQUESTS: bool = match env::var("ENABLE_GITHUB_REQUESTS") {
        Ok(val) => {
            if val == "true" {
                true
            } else {
                false
            }
        }
        Err(_) => false,
    };
    /// The Google Cloud Pubsub subscription name from which messages from github to trigger test
    /// runs will be retrieved
    pub static ref PUBSUB_SUBSCRIPTION_NAME: String = env::var("PUBSUB_SUBSCRIPTION_NAME")
        .expect("PUBSUB_SUBSCRIPTION_NAME environment variable not set");
    /// The maximum number of messages to retrieve from the pubsub subscription at once
    pub static ref PUBSUB_MAX_MESSAGES_PER: i32 = match env::var("PUBSUB_MAX_MESSAGES_PER") {
        Ok(s) => s.parse::<i32>().unwrap(),
        Err(_) => {
            info!("No PUBSUB_MAX_MESSAGES_PER specified.  Defaulting to 20 messages");
            20
        }
    };
    /// The number of time, in seconds, to wait between checks of the pubsub subscription
    pub static ref PUBSUB_WAIT_TIME_IN_SECS: u64 = match env::var("PUBSUB_WAIT_TIME_IN_SECS") {
        Ok(s) => s.parse::<u64>().unwrap(),
        Err(_) => {
            info!("No PUBSUB_WAIT_TIME_IN_SECS specified.  Defaulting to 1 minute");
            60
        }
    };

    // Building docker images from repos
    /// The host address for the docker image registry where we'll store custom docker images
    pub static ref IMAGE_REGISTRY_HOST: String =
        env::var("IMAGE_REGISTRY_HOST").expect("IMAGE_REGISTRY_HOST environment variable not set");
    /// If true, enables building docker images from private github repos
    pub static ref ENABLE_PRIVATE_GITHUB_ACCESS: bool = match env::var("ENABLE_PRIVATE_GITHUB_ACCESS") {
        Ok(val) => {
            if val == "true" {
                true
            } else {
                false
            }
        }
        Err(_) => false,
    };
    /// The github username for the user to use to access private repositories
    pub static ref PRIVATE_GITHUB_CLIENT_ID: Option<String> = match *ENABLE_PRIVATE_GITHUB_ACCESS {
        false => None,
        true => Some(env::var("PRIVATE_GITHUB_CLIENT_ID").expect("PRIVATE_GITHUB_CLIENT_ID environment variable is not set and is required if ENABLE_PRIVATE_GITHUB_ACCESS is true"))
    };
    /// The github user token for the user to use to access private repositories
    pub static ref PRIVATE_GITHUB_CLIENT_TOKEN: Option<String> = match *ENABLE_PRIVATE_GITHUB_ACCESS {
        false => None,
        true => Some(env::var("PRIVATE_GITHUB_CLIENT_TOKEN").expect("PRIVATE_GITHUB_CLIENT_TOKEN environment variable is not set and is required if ENABLE_PRIVATE_GITHUB_ACCESS is true"))
    };
    /// The GCS URL of a google kms encrypted file containing the password for the account specified by `PRIVATE_GITHUB_CLIENT_ID`
    pub static ref PRIVATE_GITHUB_CLIENT_PASS_URI: Option<String> = match *ENABLE_PRIVATE_GITHUB_ACCESS {
        false => None,
        true => Some(env::var("PRIVATE_GITHUB_CLIENT_PASS_URI").expect("PRIVATE_GITHUB_CLIENT_PASS_URI environment variable is not set and is required if ENABLE_PRIVATE_GITHUB_ACCESS is true"))
    };
    /// The GCloud KMS keyring for decrypting `PRIVATE_GITHUB_CLIENT_PASS_URI`
    pub static ref PRIVATE_GITHUB_KMS_KEYRING: Option<String> = match *ENABLE_PRIVATE_GITHUB_ACCESS {
        false => None,
        true => Some(env::var("PRIVATE_GITHUB_KMS_KEYRING").expect("PRIVATE_GITHUB_KMS_KEYRING environment variable is not set and is required if ENABLE_PRIVATE_GITHUB_ACCESS is true"))
    };
    /// The GCloud KMS key for decrypting `PRIVATE_GITHUB_CLIENT_PASS_URI`
    pub static ref PRIVATE_GITHUB_KMS_KEY: Option<String> = match *ENABLE_PRIVATE_GITHUB_ACCESS {
        false => None,
        true => Some(env::var("PRIVATE_GITHUB_KMS_KEY").expect("PRIVATE_GITHUB_KMS_KEY environment variable is not set and is required if ENABLE_PRIVATE_GITHUB_ACCESS is true"))
    };

    // Validation
    /// The location of the womtool jar to use to validate WDLs
    pub static ref WOMTOOL_LOCATION: String =
        env::var("WOMTOOL_LOCATION").expect("WOMTOOL_LOCATION environment variable not set");
        
    // Reporting
    /// GCS directory where we'll put generated report files (in the form bucket-name/my/report/directory)
    pub static ref REPORT_LOCATION: String =
        env::var("REPORT_LOCATION").expect("REPORT_LOCATION environment variable not set");
}

/// Initializes all the necessary configuration variables
pub fn initialize() {
    lazy_static::initialize(&GCLOUD_SA_KEY_FILE);

    // API Config
    lazy_static::initialize(&HOST);
    lazy_static::initialize(&PORT);

    // Database
    lazy_static::initialize(&DATABASE_URL);
    lazy_static::initialize(&DB_THREADS);

    // Cromwell
    lazy_static::initialize(&CROMWELL_ADDRESS);

    // Status-checking config
    lazy_static::initialize(&STATUS_CHECK_WAIT_TIME_IN_SECS);
    lazy_static::initialize(&ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES);

    // Emailer
    lazy_static::initialize(&EMAIL_MODE);
    lazy_static::initialize(&EMAIL_FROM);
    lazy_static::initialize(&EMAIL_DOMAIN);
    lazy_static::initialize(&EMAIL_USERNAME);
    lazy_static::initialize(&EMAIL_PASSWORD);

    // GITHUB
    lazy_static::initialize(&GITHUB_CLIENT_ID);
    lazy_static::initialize(&GITHUB_CLIENT_TOKEN);
    lazy_static::initialize(&ENABLE_GITHUB_REQUESTS);
    // If github support is enabled, initialize the other relevant config variables
    if *ENABLE_GITHUB_REQUESTS {
        lazy_static::initialize(&PUBSUB_SUBSCRIPTION_NAME);
        lazy_static::initialize(&PUBSUB_MAX_MESSAGES_PER);
        lazy_static::initialize(&PUBSUB_WAIT_TIME_IN_SECS);
    }

    // Building custom docker images
    lazy_static::initialize(&IMAGE_REGISTRY_HOST);
    lazy_static::initialize(&ENABLE_PRIVATE_GITHUB_ACCESS);
    lazy_static::initialize(&PRIVATE_GITHUB_CLIENT_ID);
    lazy_static::initialize(&PRIVATE_GITHUB_CLIENT_TOKEN);
    lazy_static::initialize(&PRIVATE_GITHUB_CLIENT_PASS_URI);
    lazy_static::initialize(&PRIVATE_GITHUB_KMS_KEYRING);
    lazy_static::initialize(&PRIVATE_GITHUB_KMS_KEY);

    // Validation
    lazy_static::initialize(&WOMTOOL_LOCATION);

    // Reporting
    lazy_static::initialize(&REPORT_LOCATION);
}
