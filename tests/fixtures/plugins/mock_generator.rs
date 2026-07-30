use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let request: serde_json::Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            let resp = serde_json::json!({
                "files": [],
                "diagnostics": [{"severity": "error", "message": e.to_string(), "path": None::<String>}],
            });
            io::stdout().write_all(resp.to_string().as_bytes()).unwrap();
            std::process::exit(1);
        }
    };

    if request["protocol_version"] != "1.0.0" {
        let version = request["protocol_version"].as_str().unwrap_or("unknown");
        let resp = serde_json::json!({
            "files": [],
            "diagnostics": [{"severity": "error", "message": format!("unsupported protocol version: expected 1.0.0, got {version}"), "path": None::<String>}],
            "unsupported_protocol_version": version,
        });
        io::stdout().write_all(resp.to_string().as_bytes()).unwrap();
        return;
    }

    let spec_name = request["spec"]["name"].as_str().unwrap_or("unknown");
    let target = request["target"].as_str().unwrap_or("unknown");
    let content = format!("target={target}\nspec={spec_name}\n");

    let resp = serde_json::json!({
        "files": [{"path": "generated.txt", "content": content}],
        "diagnostics": [],
    });

    io::stdout().write_all(resp.to_string().as_bytes()).unwrap();
}
