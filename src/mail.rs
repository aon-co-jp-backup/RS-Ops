//! SMTP経由でのOTPメール送信。[`RS-Git`](https://github.com/aon-co-jp/RS-Git)の
//! `src/mail.rs`(元は[`open-easy-web`]の`server/src/mail.rs`)と同じ設計
//! (`lettre`の同期SMTPクライアントを`spawn_blocking`でオフロード)。
//! RS-Opsは単一管理者+管理者が登録したメンバーのみを対象とするため、
//! アクセス申請通知メール(RS-Gitにある自己申請フロー用)は持たない。
//!
//! [`open-easy-web`]: https://github.com/aon-co-jp/open-easy-web

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

impl SmtpConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            host: std::env::var("RS_OPS_SMTP_HOST").ok()?,
            port: std::env::var("RS_OPS_SMTP_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(587),
            username: std::env::var("RS_OPS_SMTP_USERNAME").ok()?,
            password: std::env::var("RS_OPS_SMTP_PASSWORD").ok()?,
            from: std::env::var("RS_OPS_SMTP_FROM").ok()?,
        })
    }
}

#[derive(Debug)]
pub enum MailError {
    Build(String),
    Send(String),
}

impl std::fmt::Display for MailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MailError::Build(e) => write!(f, "email build failed: {e}"),
            MailError::Send(e) => write!(f, "email send failed: {e}"),
        }
    }
}

fn build_and_send(config: &SmtpConfig, to: &str, subject: &str, body: String) -> Result<(), MailError> {
    let email = Message::builder()
        .from(config.from.parse().map_err(|e| MailError::Build(format!("{e}")))?)
        .to(to.parse().map_err(|e| MailError::Build(format!("{e}")))?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| MailError::Build(format!("{e}")))?;

    let creds = Credentials::new(config.username.clone(), config.password.clone());
    let mailer =
        SmtpTransport::starttls_relay(&config.host).map_err(|e| MailError::Send(format!("{e}")))?.port(config.port).credentials(creds).build();

    mailer.send(&email).map_err(|e| MailError::Send(format!("{e}")))?;
    Ok(())
}

pub async fn send_otp(config: SmtpConfig, to: String, code: String) -> Result<(), MailError> {
    let body = format!(
        "RS-Ops login one-time password\n\n\
         Code: {code}\n\
         This code is valid for 5 minutes.\n\n\
         If you did not request this, you can ignore this email."
    );
    tokio::task::spawn_blocking(move || build_and_send(&config, &to, "RS-Ops login code", body))
        .await
        .map_err(|e| MailError::Send(format!("task panicked: {e}")))?
}
