use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use zegion_core::channel::{Channel, IncomingMessage, OutgoingMessage};
use zegion_core::error::Result;

/// Interactive terminal channel. Reads lines from stdin and prints replies.
#[derive(Clone)]
pub struct CliChannel {
    user_id: String,
    printed: Arc<Mutex<usize>>,
}

impl Default for CliChannel {
    fn default() -> Self {
        Self {
            user_id: "local-user".into(),
            printed: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str {
        "cli"
    }

    async fn start(&self, incoming: mpsc::Sender<IncomingMessage>) -> Result<()> {
        let user_id = self.user_id.clone();
        tokio::task::spawn_blocking(move || {
            let mut rl = rustyline::DefaultEditor::new().expect("readline init");
            while let Ok(line) = rl.readline("you> ") {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(line);
                let msg = IncomingMessage::new("cli", user_id.clone(), line);
                if incoming.blocking_send(msg).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        use std::io::Write;
        if msg.is_final {
            // If we were streaming (draft active), the deltas already printed the body;
            // just end the line. Otherwise print the full reply.
            let mut printed = self.printed.lock().unwrap();
            if *printed == 0 {
                println!("zegion> {}", msg.text);
            } else {
                println!();
                *printed = 0;
            }
            std::io::stdout().flush().ok();
        }
        Ok(())
    }

    async fn send_draft(&self, _user_id: &str, initial: &str) -> Result<Option<String>> {
        use std::io::Write;
        print!("zegion> {initial}");
        std::io::stdout().flush().ok();
        *self.printed.lock().unwrap() = initial.len();
        Ok(Some("cli".to_string()))
    }

    async fn update_draft(&self, _user_id: &str, _handle: &str, text: &str) -> Result<()> {
        use std::io::Write;
        let mut printed = self.printed.lock().unwrap();
        if text.len() > *printed {
            print!("{}", &text[*printed..]);
            std::io::stdout().flush().ok();
            *printed = text.len();
        }
        Ok(())
    }

    async fn ask_approval(&self, _user_id: &str, prompt: &str) -> Result<bool> {
        use std::io::Write;
        print!("{prompt} [y/N] ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_ok() {
            let a = line.trim().to_ascii_lowercase();
            Ok(matches!(a.as_str(), "y" | "yes"))
        } else {
            Ok(false)
        }
    }
}
