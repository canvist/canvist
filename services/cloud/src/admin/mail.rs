//! Sending sign-in links.

use std::time::Duration;

use serde_json::json;

use crate::config::Delivery;

const TIMEOUT: Duration = Duration::from_secs(10);

/// A sign-in link that could not be sent.
#[derive(Debug, thiserror::Error)]
pub enum MailError {
	/// The request never got an answer.
	#[error("could not reach the mail API: {0}")]
	Transport(String),
	/// The mail API refused the message.
	#[error("the mail API returned {0}")]
	Refused(u16),
}

/// Sends sign-in links the way the configuration says.
#[derive(Clone, Debug)]
pub struct Mailer {
	delivery: Delivery,
	http: reqwest::Client,
}

impl Mailer {
	/// A mailer for `delivery`.
	pub fn new(delivery: Delivery) -> Self {
		let http = reqwest::Client::builder()
			.timeout(TIMEOUT)
			.build()
			.expect("the mail client configuration is valid");
		Self { delivery, http }
	}

	/// Send `link`, which signs `to` in, to `to`.
	pub async fn send_sign_in(&self, to: &str, link: &str) -> Result<(), MailError> {
		let resend = match &self.delivery {
			Delivery::Log => {
				// Only reachable when the dashboard is on localhost.
				tracing::warn!(to, link, "admin sign-in link (logged, not emailed)");
				return Ok(());
			}
			Delivery::Resend(resend) => resend,
		};
		let text = format!(
			"Sign in to the canvist admin dashboard:\n\n{link}\n\nThe link works for 15 minutes. \
			 If you did not ask for it, ignore this email."
		);
		let html = format!(
			"<p>Sign in to the canvist admin dashboard:</p><p><a href=\"{link}\">Sign in to \
			 canvist</a></p><p>The link works for 15 minutes. If you did not ask for it, ignore \
			 this email.</p>",
			link = escape(link)
		);
		let response = self
			.http
			.post(format!("{}/emails", resend.api_base))
			.bearer_auth(resend.api_key.expose())
			.json(&json!({
				"from": resend.from,
				"to": [to],
				"subject": "Sign in to canvist admin",
				"text": text,
				"html": html,
			}))
			.send()
			.await
			.map_err(|error| MailError::Transport(error.to_string()))?;

		if response.status().is_success() {
			Ok(())
		} else {
			Err(MailError::Refused(response.status().as_u16()))
		}
	}
}

/// `text` safe inside an HTML attribute or element.
fn escape(text: &str) -> String {
	text.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn links_are_escaped_for_html() {
		assert_eq!(
			escape(r#"https://example.com/a?b=1&c="<x>""#),
			"https://example.com/a?b=1&amp;c=&quot;&lt;x&gt;&quot;"
		);
	}
}
