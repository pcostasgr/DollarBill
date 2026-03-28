use crate::config::AlertConfig;
use lettre::{
    message::Mailbox,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use log::{info, warn};

#[derive(Clone)]
pub struct Alerter {
    cfg: AlertConfig,
    smtp_pass: String,
}

impl Alerter {
    pub fn new(cfg: AlertConfig) -> Self {
        let smtp_pass = if cfg.smtp_password.is_empty() {
            std::env::var("DOLLARBILL_SMTP_PASSWORD").unwrap_or_default()
        } else {
            cfg.smtp_password.clone()
        };
        Self { cfg, smtp_pass }
    }

    pub fn is_active(&self) -> bool {
        self.cfg.enabled
            && !self.smtp_pass.is_empty()
            && !self.cfg.smtp_host.is_empty()
            && !self.cfg.smtp_user.is_empty()
            && !self.cfg.to.is_empty()
            && !self.cfg.from.is_empty()
    }

    /// Sends an email. Silently returns if alerting is inactive or any error occurs.
    pub async fn send(&self, subject: &str, body: &str) {
        if !self.is_active() {
            return;
        }

        let from: Mailbox = match self.cfg.from.parse() {
            Ok(m) => m,
            Err(e) => {
                warn!("Alert: invalid 'from' address: {}", e);
                return;
            }
        };
        let to: Mailbox = match self.cfg.to.parse() {
            Ok(m) => m,
            Err(e) => {
                warn!("Alert: invalid 'to' address: {}", e);
                return;
            }
        };

        let email = match Message::builder()
            .from(from)
            .to(to)
            .subject(format!("[DollarBill] {}", subject))
            .body(body.to_string())
        {
            Ok(e) => e,
            Err(e) => {
                warn!("Alert: failed to build email: {}", e);
                return;
            }
        };

        let creds = Credentials::new(self.cfg.smtp_user.clone(), self.smtp_pass.clone());

        let build_result = if self.cfg.use_smtps {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.cfg.smtp_host)
                .map(|b| b.port(self.cfg.smtp_port).credentials(creds).build())
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.cfg.smtp_host)
                .map(|b| b.port(self.cfg.smtp_port).credentials(creds).build())
        };

        match build_result {
            Ok(transport) => match transport.send(email).await {
                Ok(_) => info!("Alert email sent: {}", subject),
                Err(e) => warn!("Alert: email delivery failed: {}", e),
            },
            Err(e) => warn!("Alert: SMTP transport setup failed: {}", e),
        }
    }

    pub async fn circuit_breaker(&self, spent: f64, limit: f64) {
        if !self.cfg.on_circuit_breaker {
            return;
        }
        self.send(
            "Circuit Breaker Tripped",
            &format!(
                "Daily spend ${:.2} reached limit ${:.2}.\n\
                 No new orders will be placed until the next session.",
                spent, limit
            ),
        )
        .await;
    }

    pub async fn fill(&self, sym: &str, strategy: &str, qty: u32, price: f64) {
        if !self.cfg.on_fill {
            return;
        }
        self.send(
            &format!("Fill: {} x{} @ ${:.2}", sym, qty, price),
            &format!(
                "Symbol:   {}\nStrategy: {}\nQty:      {}\nPrice:    ${:.2}",
                sym, strategy, qty, price
            ),
        )
        .await;
    }

    pub async fn daily_loss_warning(&self, spent: f64, limit: f64) {
        if !self.cfg.on_daily_loss {
            return;
        }
        let pct = spent / limit * 100.0;
        self.send(
            &format!("Daily Loss Warning: {:.0}% of limit consumed", pct),
            &format!(
                "Estimated daily spend: ${:.2}\n\
                 Daily limit:           ${:.2}\n\
                 Consumed:              {:.1}%\n\n\
                 The circuit breaker will trip at 100%.",
                spent, limit, pct
            ),
        )
        .await;
    }

    pub async fn disconnect(&self) {
        if !self.cfg.on_disconnect {
            return;
        }
        self.send(
            "Stream Disconnected",
            "The Alpaca WebSocket stream has permanently disconnected.\n\
             The bot has stopped. Please restart it manually.",
        )
        .await;
    }
}
