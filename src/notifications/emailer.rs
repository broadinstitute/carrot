//! Contains functionality for sending notification emails to users for entities to which they've
//! subscribed

use lettre::{SmtpClient, Transport, smtp::authentication::Credentials, SendmailTransport};
use lettre_email::EmailBuilder;
use log::info;
use std::env;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

lazy_static! {
    // Get environment variable for the mode we'll use for sending mail
    pub static ref EMAIL_MODE: EmailMode = EmailMode::from_str(&env::var("EMAIL_MODE")
        .expect("EMAIL_MODE environment variable not set"))
        .expect("EMAIL_MODE must be one of three values: SERVER, SENDMAIL, or NONE");

    // Get environment variable for 'from' field in email notifications (if mode isn't None)
    static ref EMAIL_FROM: Option<String> = {
        match *EMAIL_MODE {
            EmailMode::None => None,
            _ => Some(env::var("EMAIL_FROM").expect("EMAIL_FROM environment variable not set"))
        }
    };

    // Get environment variable for domain for email server for notifications (if mode is Server)
    static ref EMAIL_DOMAIN: Option<String> = {
        match *EMAIL_MODE {
            EmailMode::Server => Some(env::var("EMAIL_DOMAIN").unwrap()),
            _ => None
        }
    };

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

/// Enum of possible email modes to be specified in env variables, corresponding to how we will or
/// will not send emails.
/// `Server` mode will send emails by connecting to a mail server
/// `Sendmail` mode will send emails using the Unix Sendmail utility
/// `None` will not send emails
#[derive(Debug)]
pub enum EmailMode {
    Server,
    Sendmail,
    None
}

impl FromStr for EmailMode {
    type Err = ParseEmailModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SERVER" => Ok(EmailMode::Server),
            "SENDMAIL" => Ok(EmailMode::Sendmail),
            "NONE" => Ok(EmailMode::None),
            _ => Err(ParseEmailModeError)
        }
    }
}

/// Error type for when parsing EMAIL_MODE fails
#[derive(Debug)]
pub struct ParseEmailModeError;

/// Enum of possible errors from sending an email
#[derive(Debug)]
pub enum SendEmailError {
    SendSMTP(lettre::smtp::error::Error),
    SendSendmail(lettre::sendmail::error::Error),
    Build(lettre_email::error::Error),
    Config(String),
}

impl fmt::Display for SendEmailError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SendEmailError::SendSMTP(e) => write!(f, "SendEmailError SendSMTP {}", e),
            SendEmailError::SendSendmail(e) => write!(f, "SendEmailError SendSendmail {}", e),
            SendEmailError::Build(e) => write!(f, "SendEmailError Build {}", e),
            SendEmailError::Config(e) => write!(f, "SendEmailError Config {}", e),
        }
    }
}

impl Error for SendEmailError {}

// Implementing From for each of the error types so they map more easily
impl From<lettre::smtp::error::Error> for SendEmailError {
    fn from(e: lettre::smtp::error::Error) -> SendEmailError {
        SendEmailError::SendSMTP(e)
    }
}
impl From<lettre::sendmail::error::Error> for SendEmailError {
    fn from(e: lettre::sendmail::error::Error) -> SendEmailError {
        SendEmailError::SendSendmail(e)
    }
}
impl From<lettre_email::error::Error> for SendEmailError {
    fn from(e: lettre_email::error::Error) -> SendEmailError {
        SendEmailError::Build(e)
    }
}

/// Initializes the (possibly-)required email-related static variables to verify that they have
/// been set correctly
///
/// lazy_static does not actually initialize variables right away. Since we're loading from env
/// variables and applying some logic when initializing them, we need to use lazy_static for the
/// email config variables.  We want to make sure they are set at runtime, though, so this
/// function initializes the ones that could possibly be required (depending on the email mode),
/// so, if the user does not set these variables properly, we can have the application panic right
/// away instead of waiting until it first tries to send an email
///
/// # Panics
/// Panics if a required environment variable is unavailable
pub fn setup() {
    lazy_static::initialize(&EMAIL_MODE);
    lazy_static::initialize(&EMAIL_FROM);
    lazy_static::initialize(&EMAIL_DOMAIN);
}

/// Sends an email to `address` with `subject` and `message`
///
/// Sends an email via an SMTP server if `EMAIL_MODE` is `Server`, via the Sendmail utility if the
/// `EMAIL_MODE` is `Sendmail`, or returns an error if the `EMAIL_MODE` is `None`.
pub fn send_email(address: &str, subject: &str, message: &str) -> Result<(), SendEmailError> {
    match *EMAIL_MODE {
        EmailMode::Server => send_email_server_mode(address, subject, message),
        EmailMode::Sendmail => send_email_sendmail_mode(address, subject, message),
        EmailMode::None => Err(SendEmailError::Config(String::from("Called send_email but EMAIL_MODE is None."))),
    }
}

/// Sends an email to `address` with `subject` and `message`
///
/// Sends an email via SMTP to the address specified by `address` with `subject` and `message`.
/// Uses the environment variable `EMAIL_FROM` for the `from` field and `EMAIL_DOMAIN` for the
/// domain of the mail server.  If values are provided in environment variables for
/// `EMAIL_USERNAME` and `EMAIL_PASSWORD`, those will be used as credentials for connecting to the
/// mail server
fn send_email_server_mode(address: &str, subject: &str, message: &str) -> Result<(), SendEmailError> {
    // Set up email to send
    let email = EmailBuilder::new()
        .to(address)
        .from((*EMAIL_FROM).clone().unwrap())
        .subject(subject)
        .text(message)
        .build()?;

    // Start to set up client for connecting to email server
    let mut mailer = SmtpClient::new_simple(&(*EMAIL_DOMAIN).clone().unwrap())
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

/// Sends an email to `address` with `subject` and `message`
///
/// Sends an email via the Sendmail utility to the address specified by `address` with `subject`
/// and `message`. Uses the environment variable `EMAIL_FROM` for the `from` field.
fn send_email_sendmail_mode(address: &str, subject: &str, message: &str) -> Result<(), SendEmailError> {
// Set up email to send
    let email = EmailBuilder::new()
        .to(address)
        .from((*EMAIL_FROM).clone().unwrap())
        .subject(subject)
        .text(message)
        .build()?;

    // Create sendmail transport to prepare to send
    let mut mailer = SendmailTransport::new();

    // Send the email
    mailer.send(email.into())?;

    Ok(())
}