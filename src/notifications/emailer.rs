//! Contains functionality for sending notification emails to users for entities to which they've
//! subscribed

use lettre::{smtp::authentication::Credentials, SendmailTransport, SmtpClient, Transport, FileTransport};
use lettre_email::{EmailBuilder, Email};
use log::info;
use std::env;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
#[cfg(test)]
use std::env::temp_dir;

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

    // Get environment variable for domain for email server for notifications
    static ref EMAIL_DOMAIN: Option<String> = {
        match *EMAIL_MODE {
            EmailMode::Server => Some(env::var("EMAIL_DOMAIN").expect("EMAIL_DOMAIN environment variable not set")),
            _ => None,
        }
    };

    // Get environment variable for server username if it exists
    static ref EMAIL_USERNAME: Option<String> = {
        match env::var("EMAIL_USERNAME") {
            Ok(s) => Some(s),
            Err(_) =>  {
                info!("No value specified for EMAIL_USERNAME");
                None
            }
        }
    };

    // Get environment variable for server password if it exists
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
    None,
}

impl FromStr for EmailMode {
    type Err = ParseEmailModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SERVER" => Ok(EmailMode::Server),
            "SENDMAIL" => Ok(EmailMode::Sendmail),
            "NONE" => Ok(EmailMode::None),
            _ => Err(ParseEmailModeError),
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
    File(lettre::file::error::Error),
}

impl fmt::Display for SendEmailError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SendEmailError::SendSMTP(e) => write!(f, "SendEmailError SendSMTP {}", e),
            SendEmailError::SendSendmail(e) => write!(f, "SendEmailError SendSendmail {}", e),
            SendEmailError::Build(e) => write!(f, "SendEmailError Build {}", e),
            SendEmailError::Config(e) => write!(f, "SendEmailError Config {}", e),
            SendEmailError::File(e) => write!(f, "SendEmailError File {}", e),
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

impl From<lettre::file::error::Error> for SendEmailError {
    fn from(e: lettre::file::error::Error) -> SendEmailError {
        SendEmailError::File(e)
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

/// Sends an email bcc'd to `addresses` with `subject` and `message`
///
/// Sends an email via an SMTP server if `EMAIL_MODE` is `Server`, via the Sendmail utility if the
/// `EMAIL_MODE` is `Sendmail`, or returns an error if the `EMAIL_MODE` is `None`.
pub fn send_email(addresses: Vec<&str>, subject: &str, message: &str) -> Result<(), SendEmailError> {

    // Set up email to send
    let email = build_email(&addresses, subject, message)?;

    // Send email based on email mode
    #[cfg(not(test))]
    match *EMAIL_MODE {
        EmailMode::Server => send_email_server_mode(email),
        EmailMode::Sendmail => send_email_sendmail_mode(email),
        EmailMode::None => Err(SendEmailError::Config(String::from(
            "Called send_email but EMAIL_MODE is None.",
        ))),
    }

    // If this is a test, print the email to a file
    // Note: Some IDEs (or, Intellij, anyway) incorrectly mark a syntax error here because they
    // think `email` is being used here after being moved above.  However, this statement and the
    // statement above will never be included in the same build (this one is only for test builds,
    // and the one above is for all others), so there is no actual syntax error here.
    #[cfg(test)]
    {
        let dir: &str = addresses[0].split("@").collect::<Vec<&str>>()[0];
        send_email_test_mode(email, dir)
    }

}

/// Assembles and returns a lettre email based on `address`, `subject`, and `message`
fn build_email(addresses: &Vec<&str>, subject: &str, message: &str) -> Result<Email, lettre_email::error::Error> {

    // Set up email to send
    let mut email = EmailBuilder::new()
        .from((*EMAIL_FROM).clone().unwrap())
        .subject(subject)
        .text(message);

    for address in addresses {
        email = email.bcc(*address)
    }

    let email = email.build()?;

    Ok(email)
}

/// Sends email defined by `email` via an SMTP server
///
/// Uses the environment variable `EMAIL_DOMAIN` for the domain of the mail server.  If values are
/// provided in environment variables for `EMAIL_USERNAME` and `EMAIL_PASSWORD`, those will be
/// used as credentials for connecting to the mail server
fn send_email_server_mode(
    email: Email
) -> Result<(), SendEmailError> {

    // Start to set up client for connecting to email server
    let mut mailer = SmtpClient::new_simple(&(*EMAIL_DOMAIN).clone().unwrap())
        .expect("Failed to create smtp client for sending email");

    // If we have credentials, add those to the client setup
    if (*EMAIL_USERNAME).is_some() && (*EMAIL_PASSWORD).is_some() {
        mailer = mailer.credentials(Credentials::new(
            (*EMAIL_USERNAME).clone().unwrap(),
            (*EMAIL_PASSWORD).clone().unwrap(),
        ));
    }

    // Convert to transport to prepare to send
    let mut mailer = mailer.transport();

    // Send the email
    mailer.send(email.into())?;

    Ok(())
}

/// Sends email defined by `email` via the Sendmail utility.
fn send_email_sendmail_mode(
    email: Email
) -> Result<(), SendEmailError> {

    // Create sendmail transport to prepare to send
    let mut mailer = SendmailTransport::new();

    // Send the email
    mailer.send(email.into())?;

    Ok(())
}

// Test function that prints email to file instead of sending it
#[cfg(test)]
fn send_email_test_mode(
    email: Email,
    dir: &str
) -> Result<(), SendEmailError> {

    let mut dir_path = temp_dir();
    dir_path.push(dir);

    // Create sendmail transport to prepare to send
    let mut mailer = FileTransport::new(dir_path);

    // Send the email
    mailer.send(email.into())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::notifications::emailer::send_email;
    use tempfile::Builder;
    use std::fs::{read_dir, DirEntry, read_to_string};
    use serde_json::Value;
    use serde::Deserialize;
    use mailparse::MailHeaderMap;
    use std::env::temp_dir;

    #[derive(Deserialize)]
    struct ParsedEmailFile {
        envelope: Value,
        #[serde(with = "serde_bytes")]
        message: Vec<u8>
    }

    #[test]
    fn test_send_email_success() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        // Create temporary directory for file
        let mut dir_path = Builder::new()
            .prefix("test_send_email")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        let test_address = "test_send_email@example.com";
        let test_subject = "Test Subject";
        let test_message = "This is a test message";

        if let Err(e) = send_email(vec![test_address], test_subject, test_message){
            panic!("Send email failed with error: {}", e);
        };

        // Read the file
        let files_in_dir = read_dir(dir_path.path()).unwrap().collect::<Vec<std::io::Result<DirEntry>>>();

        assert_eq!(files_in_dir.len(), 1);

        let test_email_string = read_to_string(files_in_dir.get(0).unwrap().as_ref().unwrap().path()).unwrap();
        let test_email: ParsedEmailFile = serde_json::from_str(&test_email_string).unwrap();

        assert_eq!(test_email.envelope.get("forward_path").unwrap().as_array().unwrap().get(0).unwrap(), test_address);
        assert_eq!(test_email.envelope.get("reverse_path").unwrap(), "kevin@example.com");

        let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

        assert_eq!(parsed_mail.subparts[0].get_body().unwrap().trim(), test_message);
        assert_eq!(parsed_mail.headers.get_first_value("Subject").unwrap(), test_subject);

        dir_path.close().unwrap();
    }

    #[test]
    fn test_send_email_failure_bad_email() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        let test_addresses = vec!["t@es@t_s@end_@email@example.com"];
        let test_subject = "Test Subject";
        let test_message = "This is a test message";

        match send_email(test_addresses, test_subject, test_message){
            Err(e) => {
                match e {
                    super::SendEmailError::Build(_) => {},
                    _ => panic!("Send email failed with unexpected error: {}", e)
                }
            },
            _ => panic!("Send email succeeded unexpectedly")
        }
    }


}
