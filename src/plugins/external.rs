use crate::error::PluginError;
use crate::plugins::protocol::{GenerateRequest, GenerateResponse, PluginDiagnostic, PROTOCOL_VERSION};
use crate::traits::Plugin;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const RESPONSE_SIZE_LIMIT: usize = 50 * 1024 * 1024;
#[allow(dead_code)]
const PLUGIN_TIMEOUT: Duration = Duration::from_secs(120);

/// An external generator plugin backed by a subprocess binary.
pub struct ExternalPlugin {
    target: String,
    name: String,
    executable: PathBuf,
}

impl ExternalPlugin {
    pub fn new(target: impl Into<String>, executable: impl Into<PathBuf>) -> Self {
        let target = target.into();
        let executable = executable.into();
        // Name defaults to the target, but we try to read a file stem for clarity.
        let name = executable
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| target.clone());
        Self {
            target,
            name,
            executable,
        }
    }

    fn sanitized_env(&self) -> Vec<(String, String)> {
        let allowed = ["PATH", "HOME", "WEBSPEC_PLUGIN_DIR", "RUST_LOG", "TMPDIR", "TEMP", "USERPROFILE"];
        std::env::vars()
            .filter(|(k, _)| allowed.contains(&k.as_str()))
            .collect()
    }

    fn validate_response(
        &self,
        response: &GenerateResponse,
    ) -> Result<(), PluginError> {
        if let Some(version) = &response.unsupported_protocol_version {
            return Err(PluginError::unsupported_protocol(&self.target, version));
        }

        if response
            .diagnostics
            .iter()
            .any(|d| d.severity == "error")
        {
            let message = response
                .diagnostics
                .iter()
                .filter(|d| d.severity == "error")
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(PluginError::diagnostic_error(&self.target, message));
        }

        for file in &response.files {
            Self::validate_file_path(&file.path)?;
        }

        Ok(())
    }

    fn validate_file_path(path: &str) -> Result<(), PluginError> {
        if Path::new(path).is_absolute() {
            return Err(PluginError::invalid_path("external", format!("absolute path: {path}")));
        }
        if path.contains("..") {
            return Err(PluginError::invalid_path("external", format!("path traversal: {path}")));
        }
        Ok(())
    }
}

impl Plugin for ExternalPlugin {
    fn target(&self) -> &str {
        &self.target
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, PluginError> {
        let input = serde_json::to_vec(request)
            .map_err(|e| PluginError::process_failed(&self.target, e.to_string()))?;

        let env_pairs = self.sanitized_env();

        let mut command = Command::new(&self.executable);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        command.env_clear();
        for (k, v) in env_pairs {
            command.env(k, v);
        }

        let mut child = command
            .spawn()
            .map_err(|e| PluginError::process_failed(&self.target, format!("failed to spawn: {e}")))?;

        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| PluginError::process_failed(&self.target, "no stdin"))?;
            std::io::Write::write_all(&mut stdin, &input)
                .map_err(|e| PluginError::process_failed(&self.target, format!("stdin write: {e}")))?;
        }

        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => return Err(PluginError::process_failed(&self.target, format!("{e}"))),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PluginError::process_failed(
                &self.target,
                format!("plugin exited with {}: {stderr}", output.status),
            ));
        }

        if output.stdout.len() > RESPONSE_SIZE_LIMIT {
            return Err(PluginError::OutputTooLarge {
                target: self.target.clone(),
            });
        }

        let response: GenerateResponse = serde_json::from_slice(&output.stdout)
            .map_err(|e| PluginError::process_failed(&self.target, format!("invalid JSON response: {e}")))?;

        self.validate_response(&response)?;
        Ok(response)
    }
}

/// Serialize diagnostics to a JSON response with a protocol-mismatch indicator.
pub fn unsupported_protocol_response(version: &str) -> GenerateResponse {
    GenerateResponse {
        files: Vec::new(),
        diagnostics: vec![PluginDiagnostic {
            severity: "error".into(),
            message: format!("unsupported protocol version: expected {PROTOCOL_VERSION}, got {version}"),
            path: None,
        }],
        unsupported_protocol_version: Some(version.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absolute_path() {
        let err = ExternalPlugin::validate_file_path("/etc/passwd").unwrap_err();
        assert!(err.to_string().contains("absolute path"));
    }

    #[test]
    fn rejects_path_traversal() {
        let err = ExternalPlugin::validate_file_path("../../secret").unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn accepts_relative_path() {
        ExternalPlugin::validate_file_path("src/lib.rs").unwrap();
    }

    #[test]
    fn unsupported_protocol_response_has_flag() {
        let resp = unsupported_protocol_response("0.9.0");
        assert_eq!(resp.unsupported_protocol_version, Some("0.9.0".into()));
        assert_eq!(resp.diagnostics[0].severity, "error");
    }
}
