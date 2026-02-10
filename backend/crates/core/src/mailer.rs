use crate::models::Tenant;
use lettre::{
    message::{header::ContentType, SinglePart},
    transport::smtp::authentication::Credentials,
    transport::smtp::client::Tls,
    transport::smtp::client::TlsParameters,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum MailerError {
    #[error("SMTP configuration missing")]
    ConfigurationMissing,
    #[error("Failed to build email: {0}")]
    BuildError(#[from] lettre::error::Error),
    #[error("Failed to send email: {0}")]
    SendError(#[from] lettre::transport::smtp::Error),
    #[error("Invalid email address: {0}")]
    AddressError(#[from] lettre::address::AddressError),
}

pub async fn send_email(
    tenant: &Tenant,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), MailerError> {
    let (host, port, username, password, from) = match (
        &tenant.smtp_host,
        tenant.smtp_port,
        &tenant.smtp_username,
        &tenant.smtp_password,
        &tenant.smtp_from,
    ) {
        (Some(h), Some(p), Some(u), Some(pw), Some(f)) => (h, p, u, pw, f),
        _ => return Err(MailerError::ConfigurationMissing),
    };

    let email = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(body.to_string()),
        )?;

    let creds = Credentials::new(username.clone(), password.clone());
    let secure = tenant.smtp_secure.unwrap_or(true);
    let port_u16 = port as u16;

    let mailer: AsyncSmtpTransport<Tokio1Executor> = if port_u16 == 465 && secure {
        // Port 465: Implicit TLS (TLS wrapping from the start)
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port_u16)
            .credentials(creds)
            .tls(Tls::Wrapper(TlsParameters::new(host.clone())?))
            .timeout(Some(Duration::from_secs(10)))
            .build()
    } else if secure {
        // Port 587 or other with secure=true: STARTTLS
        // relay() configures Tls::Required internally
        AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
            .port(port_u16)
            .credentials(creds)
            .timeout(Some(Duration::from_secs(10)))
            .build()
    } else {
        // secure=false: No TLS
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port_u16)
            .credentials(creds)
            .tls(Tls::None)
            .timeout(Some(Duration::from_secs(10)))
            .build()
    };

    mailer.send(email).await?;

    Ok(())
}

pub async fn test_smtp_connection(
    host: &str,
    port: i32,
    username: &str,
    password: &str,
    secure: bool,
) -> Result<(), MailerError> {
    let creds = Credentials::new(username.to_string(), password.to_string());
    let port_u16 = port as u16;

    let mailer: AsyncSmtpTransport<Tokio1Executor> = if port_u16 == 465 && secure {
        // Port 465: Implicit TLS
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port_u16)
            .credentials(creds)
            .tls(Tls::Wrapper(TlsParameters::new(host.to_string())?))
            .timeout(Some(Duration::from_secs(5)))
            .build()
    } else if secure {
        // STARTTLS
        AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
            .port(port_u16)
            .credentials(creds)
            .timeout(Some(Duration::from_secs(5)))
            .build()
    } else {
        // No TLS
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port_u16)
            .credentials(creds)
            .tls(Tls::None)
            .timeout(Some(Duration::from_secs(5)))
            .build()
    };

    mailer.test_connection().await?;

    Ok(())
}
