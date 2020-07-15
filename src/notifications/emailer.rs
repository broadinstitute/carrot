//! Contains functionality for sending notification emails to users for entities to which they've
//! subscribed
//!

use lettre::{SmtpClient, Transport, smtp::authentication::Credentials};
use lettre_email::EmailBuilder;
use log::{info, error};

pub fn send_email(address: &str, subject: &str, message: &str) -> Result<(), lettre_email::error::Error> {
    let email = EmailBuilder::new()
        .to(address)
        .from("")
        .subject(subject)
        .text(message)
        .build()?;

    let mut mailer = SmtpClient::new_simple("smtp.gmail.com")
        .expect("Failed to create smtp client")
        .credentials(Credentials::new("".to_string(), "".to_string()))
        .transport();

    let result = mailer.send(email.into());

    if result.is_ok() {
        info!("Sent email");
    }
    else {
        error!("Could not send email: {:?}", result);
    }

    Ok(())
}