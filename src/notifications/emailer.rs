//! Contains functionality for sending notification emails to users for entities to which they've
//! subscribed
//!

use lettre::{SmtpClient, Transport, smtp::authentication::Credentials};
use lettre_email::EmailBuilder;
use log::info;
use std::env;
use std::error::Error;
use std::fmt;

lazy_static! {
    // Get environment variable for 'from' field in email notifications
    static ref EMAIL_FROM: String = env::var("EMAIL_FROM").unwrap();

    // Get environment variable for domain for email server for notifications
    static ref EMAIL_DOMAIN: String = env::var("EMAIL_DOMAIN").unwrap();

    // Get environment variable for domain for email server for notifications
    static ref EMAIL_USERNAME: Option<String> = {
        match env::var("EMAIL_USERNAME") {
            Ok(s) => Some(s),
            Err(_) =>  {
                info!("No value specified for EMAIL_USERNAME");
                None
            }
        }
    };

    // Get environment variable for domain for email server for notifications
    static ref EMAIL_PASSWORD: Option<String> = {
        match env::var("EMAIL_PASSWORD") {
            Ok(s) => Some(s),
            Err(_) =>  {
                info!("No value specified for EMAIL_PASSWORD");
                None
            }
        }
    };
}

/// Enum of possible errors from sending an email
#[derive(Debug)]
pub enum SendEmailError {
    Send(lettre::smtp::error::Error),
    Build(lettre_email::error::Error)
}

impl fmt::Display for SendEmailError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SendEmailError::Send(e) => write!(f, "SendEmailError Send {}", e),
            SendEmailError::Build(e) => write!(f, "SendEmailError Build {}", e),
        }
    }
}

impl Error for SendEmailError {}

// Implementing From for each of the error types so they map more easily
impl From<lettre::smtp::error::Error> for SendEmailError {
    fn from(e: lettre::smtp::error::Error) -> SendEmailError {
        SendEmailError::Send(e)
    }
}
impl From<lettre_email::error::Error> for SendEmailError {
    fn from(e: lettre_email::error::Error) -> SendEmailError {
        SendEmailError::Build(e)
    }
}

/// Sends an email to `address` with `subject` and `message`
///
/// Sends an email via SMTP to the address specified by `address` with `subject` and `message`.
/// Uses the environment variable `EMAIL_FROM` for the `from` field and `EMAIL_DOMAIN` for the
/// domain of the mail server.  If values are provided in environment variables for
/// `EMAIL_USERNAME` and `EMAIL_PASSWORD`, those will be used as credentials for connecting to the
/// mail server
pub fn send_email(address: &str, subject: &str, message: &str) -> Result<(), SendEmailError> {

    // Set up email to send
    let email = EmailBuilder::new()
        .to(address)
        .from((*EMAIL_FROM).clone())
        .subject(subject)
        .text(message)
        .build()?;

    // Start to set up client for connecting to email server
    let mut mailer = SmtpClient::new_simple(&*EMAIL_DOMAIN)
        .expect("Failed to create smtp client for sending email");

    // If we have credentials, add those to the client setup
    if (*EMAIL_USERNAME).is_some() && (*EMAIL_PASSWORD).is_some() {
        mailer = mailer.credentials(Credentials::new((*EMAIL_USERNAME).clone().unwrap(), (*EMAIL_PASSWORD).clone().unwrap()));
    }

    // Convert to transport to prepare to send
    let mut mailer = mailer.transport();

    // Send the email
    mailer.send(email.into())?;

    Ok(())
}