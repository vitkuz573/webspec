use std::fmt;

#[derive(Debug, Clone, Default)]
pub struct LossReport {
    pub warnings: Vec<String>,
}

impl LossReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub fn unsupported(&mut self, feature: &str, detail: &str) {
        self.warnings.push(format!("unsupported {feature}: {detail}"));
    }

    pub fn merge(&mut self, other: LossReport) {
        self.warnings.extend(other.warnings);
    }
}

impl fmt::Display for LossReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for w in &self.warnings {
            writeln!(f, "warning: {}", w)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_warnings() {
        let mut report = LossReport::new();
        report.warn("request bodies are not represented in webspec");
        report.unsupported("security scopes", "OAuth2 scopes are dropped");
        assert_eq!(report.warnings.len(), 2);
    }
}
