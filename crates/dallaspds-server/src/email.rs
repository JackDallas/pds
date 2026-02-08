use dallaspds_core::config::SmtpConfig;
use dallaspds_core::{PdsError, PdsResult};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
};

pub struct EmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from_address: String,
}

impl EmailSender {
    pub fn new(config: &SmtpConfig) -> PdsResult<Self> {
        let creds = Credentials::new(config.username.clone(), config.password.clone());
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|e| PdsError::InternalError(format!("SMTP relay error: {e}")))?
            .port(config.port)
            .credentials(creds)
            .build();
        Ok(Self {
            transport,
            from_address: config.from_address.clone(),
        })
    }

    pub async fn send_verification_email(
        &self,
        to: &str,
        token: &str,
        pds_url: &str,
    ) -> PdsResult<()> {
        let body = format!(
            "Your verification code is: {token}\n\nOr visit: {pds_url}/xrpc/com.atproto.server.confirmEmail?token={token}"
        );
        self.send_email(to, "Verify your email address", &body).await
    }

    pub async fn send_password_reset_email(
        &self,
        to: &str,
        token: &str,
        pds_url: &str,
    ) -> PdsResult<()> {
        let body = format!(
            "Your password reset token is: {token}\n\nOr visit: {pds_url}/reset-password?token={token}"
        );
        self.send_email(to, "Password Reset Request", &body).await
    }

    pub async fn send_email_update_email(
        &self,
        to: &str,
        token: &str,
        pds_url: &str,
    ) -> PdsResult<()> {
        let body = format!(
            "Your email update confirmation token is: {token}\n\nOr visit: {pds_url}/update-email?token={token}"
        );
        self.send_email(to, "Email Update Confirmation", &body)
            .await
    }

    async fn send_email(&self, to: &str, subject: &str, body: &str) -> PdsResult<()> {
        let email = Message::builder()
            .from(
                self.from_address
                    .parse()
                    .map_err(|e| PdsError::InternalError(format!("Invalid from address: {e}")))?,
            )
            .to(to
                .parse()
                .map_err(|e| PdsError::InternalError(format!("Invalid to address: {e}")))?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())
            .map_err(|e| PdsError::InternalError(format!("Failed to build email: {e}")))?;

        self.transport
            .send(email)
            .await
            .map_err(|e| PdsError::InternalError(format!("Failed to send email: {e}")))?;
        Ok(())
    }
}
