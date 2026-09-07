use crate::error::{Error, Result};

/// The outcome of a guardrail inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Block(String),
    Flag(String),
}

/// A guardrail inspects LLM input (the prompt/messages) or output (the reply) and
/// decides whether it may proceed.
pub trait Guardrail: Send + Sync {
    fn name(&self) -> &str;
    /// Inspect text in the given direction and return a verdict.
    fn inspect(&self, text: &str, direction: Direction) -> Verdict;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// Compose several guardrails; the first `Block` wins, otherwise the strictest verdict.
pub struct GuardrailSet {
    rails: Vec<Box<dyn Guardrail>>,
}

impl GuardrailSet {
    pub fn new(rails: Vec<Box<dyn Guardrail>>) -> Self {
        Self { rails }
    }

    /// Check input before sending to the model. Returns Err on Block.
    pub fn check_input(&self, text: &str) -> Result<()> {
        self.run(text, Direction::Input)
    }

    /// Check model output before returning to the user. Returns Err on Block.
    pub fn check_output(&self, text: &str) -> Result<()> {
        self.run(text, Direction::Output)
    }

    fn run(&self, text: &str, direction: Direction) -> Result<()> {
        for rail in &self.rails {
            match rail.inspect(text, direction) {
                Verdict::Block(reason) => {
                    return Err(Error::Provider(format!(
                        "guardrail `{}` blocked {}: {reason}",
                        rail.name(),
                        direction_str(direction)
                    )));
                }
                Verdict::Flag(reason) => {
                    tracing::warn!("guardrail `{}` flagged: {reason}", rail.name());
                }
                Verdict::Allow => {}
            }
        }
        Ok(())
    }
}

fn direction_str(d: Direction) -> &'static str {
    match d {
        Direction::Input => "input",
        Direction::Output => "output",
    }
}

/// Blocks or flags obviously sensitive content in either direction.
pub struct SensitiveDataGuardrail {
    patterns: Vec<&'static str>,
}

impl Default for SensitiveDataGuardrail {
    fn default() -> Self {
        Self {
            patterns: vec![
                "-----BEGIN",
                "PRIVATE KEY-----",
                "aws_secret_access_key",
                "BEGIN RSA PRIVATE",
                "BEGIN OPENSSH PRIVATE",
            ],
        }
    }
}

impl Guardrail for SensitiveDataGuardrail {
    fn name(&self) -> &str {
        "sensitive-data"
    }
    fn inspect(&self, text: &str, _direction: Direction) -> Verdict {
        let lower = text.to_ascii_lowercase();
        for p in &self.patterns {
            if lower.contains(&p.to_ascii_lowercase()) {
                return Verdict::Block(format!("contains sensitive pattern `{p}`"));
            }
        }
        Verdict::Allow
    }
}

/// Flags unusually long inputs/outputs as a soft budget guard.
pub struct LengthGuardrail {
    pub max_chars: usize,
}

impl Guardrail for LengthGuardrail {
    fn name(&self) -> &str {
        "length"
    }
    fn inspect(&self, text: &str, _direction: Direction) -> Verdict {
        if text.len() > self.max_chars {
            Verdict::Flag(format!(
                "length {} exceeds soft cap {}",
                text.len(),
                self.max_chars
            ))
        } else {
            Verdict::Allow
        }
    }
}

/// A reasonable default set: sensitive-data blocking + a generous length flag.
pub fn default_guardrails() -> GuardrailSet {
    GuardrailSet::new(vec![
        Box::new(SensitiveDataGuardrail::default()),
        Box::new(LengthGuardrail { max_chars: 32_000 }),
    ])
}
