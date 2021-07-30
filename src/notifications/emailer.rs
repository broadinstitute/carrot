//! Contains functionality for sending notification emails to users for entities to which they've
//! subscribed

use crate::config::EmailConfig;
#[cfg(test)]
use lettre::FileTransport;
use lettre::{smtp::authentication::Credentials, SendmailTransport, SmtpClient, Transport};
use lettre_email::{Email, EmailBuilder};
#[cfg(test)]
use std::env::temp_dir;
use std::fmt;

/// Struct for sending emails, based on an `EmailConfig`
pub struct Emailer {
    config: EmailConfig,
}

/// Enum of possible errors from sending an email
#[derive(Debug)]
pub enum Error {
    SendSMTP(lettre::smtp::error::Error),
    SendSendmail(lettre::sendmail::error::Error),
    Build(lettre_email::error::Error),
    File(lettre::file::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::SendSMTP(e) => write!(f, "Emailer Error SendSMTP {}", e),
            Error::SendSendmail(e) => write!(f, "Emailer Error SendSendmail {}", e),
            Error::Build(e) => write!(f, "Emailer Error Build {}", e),
            Error::File(e) => write!(f, "Emailer Error File {}", e),
        }
    }
}

impl std::error::Error for Error {}

// Implementing From for each of the error types so they map more easily
impl From<lettre::smtp::error::Error> for Error {
    fn from(e: lettre::smtp::error::Error) -> Error {
        Error::SendSMTP(e)
    }
}
impl From<lettre::sendmail::error::Error> for Error {
    fn from(e: lettre::sendmail::error::Error) -> Error {
        Error::SendSendmail(e)
    }
}
impl From<lettre_email::error::Error> for Error {
    fn from(e: lettre_email::error::Error) -> Error {
        Error::Build(e)
    }
}
impl From<lettre::file::error::Error> for Error {
    fn from(e: lettre::file::error::Error) -> Error {
        Error::File(e)
    }
}

impl Emailer {
    /// Create a new emailer with the specified config
    pub fn new(config: EmailConfig) -> Emailer {
        Emailer { config }
    }

    /// Sends an email bcc'd to `addresses` with `subject` and `message`
    ///
    /// Sends an email via an SMTP server if `self.config` is `Server`, or via the Sendmail utility if
    /// it is `Sendmail`.
    pub fn send_email(
        &self,
        addresses: Vec<&str>,
        subject: &str,
        message: &str,
    ) -> Result<(), Error> {
        // Send email based on email config
        #[cfg(not(test))]
        match &self.config {
            EmailConfig::Server(email_server_config) => {
                // Set up email to send
                let email =
                    Emailer::build_email(&addresses, subject, message, email_server_config.from())?;
                // Send it
                Emailer::send_email_server_mode(
                    email,
                    &email_server_config.domain(),
                    email_server_config.username(),
                    email_server_config.password(),
                )
            }
            EmailConfig::Sendmail(email_sendmail_config) => {
                // Set up email to send
                let email = Emailer::build_email(
                    &addresses,
                    subject,
                    message,
                    email_sendmail_config.from(),
                )?;
                // Send it
                Emailer::send_email_sendmail_mode(email)
            }
        }

        // If this is a test, print the email to a file
        // Note: Some IDEs (or, Intellij, anyway) incorrectly mark a syntax error here because they
        // think `email` is being used here after being moved above.  However, this statement and the
        // statement above will never be included in the same build (this one is only for test builds,
        // and the one above is for all others), so there is no actual syntax error here.
        #[cfg(test)]
        {
            let from = match &self.config {
                EmailConfig::Server(email_server_config) => email_server_config.from().to_owned(),
                EmailConfig::Sendmail(email_sendmail_config) => {
                    email_sendmail_config.from().to_owned()
                }
            };
            let email = Emailer::build_email(&addresses, subject, message, &from)?;
            let dir: &str = addresses[0].split("@").collect::<Vec<&str>>()[0];
            Emailer::send_email_test_mode(email, dir)
        }
    }

    /// Assembles and returns a lettre email based on `addresses`, `subject`, `message`, and `from`.
    /// Values in `addresses` will be bcc'd (instead of just sticking them all in the `to` field)
    fn build_email(
        addresses: &Vec<&str>,
        subject: &str,
        message: &str,
        from: &str,
    ) -> Result<Email, lettre_email::error::Error> {
        // Set up email to send
        let mut email = EmailBuilder::new()
            .from(from)
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
    /// Uses the domain from `email_server_config` for the domain of the mail server.  If values are
    /// provided for username and password in `email_server_config`, those will be used as credentials
    /// for connecting to the mail server
    fn send_email_server_mode(
        email: Email,
        domain: &str,
        username: Option<&String>,
        password: Option<&String>,
    ) -> Result<(), Error> {
        // Start to set up client for connecting to email server
        let mut mailer =
            SmtpClient::new_simple(domain).expect("Failed to create smtp client for sending email");

        // If we have credentials, add those to the client setup
        if username.is_some() && password.is_some() {
            mailer = mailer.credentials(Credentials::new(
                String::from(username.unwrap()),
                String::from(password.unwrap()),
            ));
        }

        // Convert to transport to prepare to send
        let mut mailer = mailer.transport();

        // Send the email
        mailer.send(email.into())?;

        Ok(())
    }

    /// Sends email defined by `email` via the Sendmail utility.
    fn send_email_sendmail_mode(email: Email) -> Result<(), Error> {
        // Create sendmail transport to prepare to send
        let mut mailer = SendmailTransport::new();

        // Send the email
        mailer.send(email.into())?;

        Ok(())
    }

    // Test function that prints email to file instead of sending it
    #[cfg(test)]
    fn send_email_test_mode(email: Email, dir: &str) -> Result<(), Error> {
        let mut dir_path = temp_dir();
        dir_path.push(dir);

        // Create file transport to prepare to send
        let mut mailer = FileTransport::new(dir_path);

        // Send the email
        mailer.send(email.into())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{EmailConfig, EmailSendmailConfig};
    use crate::notifications::emailer::Emailer;
    use mailparse::MailHeaderMap;
    use serde::Deserialize;
    use serde_json::Value;
    use std::env::temp_dir;
    use std::fs::{read_dir, read_to_string, DirEntry};
    use tempfile::Builder;

    #[derive(Deserialize)]
    struct ParsedEmailFile {
        envelope: Value,
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
    }

    #[test]
    fn test_send_email_success() {
        // Make a test email config
        let email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));

        // Make a test emailer
        let test_emailer = Emailer::new(email_config);

        // Create temporary directory for file
        let dir_path = Builder::new()
            .prefix("test_send_email")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        let test_address = "test_send_email@example.com";
        let test_subject = "Test Subject";
        let test_message = "This is a test message";

        if let Err(e) = test_emailer.send_email(vec![test_address], test_subject, test_message) {
            panic!("Send email failed with error: {}", e);
        };

        // Read the file
        let files_in_dir = read_dir(dir_path.path())
            .unwrap()
            .collect::<Vec<std::io::Result<DirEntry>>>();

        assert_eq!(files_in_dir.len(), 1);

        let test_email_string =
            read_to_string(files_in_dir.get(0).unwrap().as_ref().unwrap().path()).unwrap();
        let test_email: ParsedEmailFile = serde_json::from_str(&test_email_string).unwrap();

        assert_eq!(
            test_email
                .envelope
                .get("forward_path")
                .unwrap()
                .as_array()
                .unwrap()
                .get(0)
                .unwrap(),
            test_address
        );
        assert_eq!(
            test_email.envelope.get("reverse_path").unwrap(),
            "kevin@example.com"
        );

        let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

        assert_eq!(
            parsed_mail.subparts[0].get_body().unwrap().trim(),
            test_message
        );
        assert_eq!(
            parsed_mail.headers.get_first_value("Subject").unwrap(),
            test_subject
        );

        dir_path.close().unwrap();
    }

    #[test]
    fn test_send_email_failure_bad_email() {
        // Make a test email config
        let email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));

        // Make a test emailer
        let test_emailer = Emailer::new(email_config);

        let test_addresses = vec!["t@es@t_s@end_@email@example.com"];
        let test_subject = "Test Subject";
        let test_message = "This is a test message";

        match test_emailer.send_email(test_addresses, test_subject, test_message) {
            Err(e) => match e {
                super::Error::Build(_) => {}
                _ => panic!("Send email failed with unexpected error: {}", e),
            },
            _ => panic!("Send email succeeded unexpectedly"),
        }
    }
}
