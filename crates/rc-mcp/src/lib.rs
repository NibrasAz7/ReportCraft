//! `rc-mcp` — ReportCraft Model Context Protocol (MCP) server over stdio.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

use std::io::{BufRead, Write};

use rc_engine::Session;
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED: &[&str] = &[PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

const INSTRUCTIONS: &str = "ReportCraft is a Rust-native business-intelligence engine. Use `execute` \
with any command id from `list_commands` (`table.import_csv`, `model.relate`, `model.add_measure`, \
`model.query`, `visual.add`, `report.export_svg`, `report.export_html`), or inspect \
`reportcraft://model` for the semantic model summary.";

#[derive(Default)]
pub struct Server {
    session: Session,
}

impl Server {
    pub fn new() -> Self {
        Server { session: Session::new() }
    }

    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if let Some(reply) = self.handle_line(&line) {
                output.write_all(reply.as_bytes())?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(msg) => self.handle(msg),
            Err(e) => Some(error(Value::Null, PARSE_ERROR, format!("parse error: {e}"))),
        };
        reply.map(|r| r.to_string())
    }

    fn handle(&mut self, msg: Value) -> Option<Value> {
        let Value::Object(o) = &msg else {
            return Some(error(Value::Null, INVALID_REQUEST, "message must be an object"));
        };
        let id = o.get("id").cloned();
        let Some(method) = o.get("method").and_then(Value::as_str) else {
            return Some(error(id.unwrap_or(Value::Null), INVALID_REQUEST, "missing `method`"));
        };
        let params = o.get("params").cloned().unwrap_or(Value::Null);
        let id = id?;
        Some(match self.request(method, &params) {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, m)) => error(id, code, m),
        })
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "ping" => Ok(json!({})),
            "initialize" => {
                let client_v = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION);
                let version = if SUPPORTED.contains(&client_v) { client_v } else { PROTOCOL_VERSION };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}, "resources": {}},
                    "serverInfo": {"name": "reportcraft", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                }))
            }
            "tools/list" => Ok(json!({
                "tools": [
                    {
                        "name": "execute",
                        "description": "Execute any ReportCraft engine command by id with JSON params.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "command": {"type": "string"},
                                "params": {"type": "object"}
                            },
                            "required": ["command"]
                        }
                    },
                    {
                        "name": "list_commands",
                        "description": "List all ReportCraft engine commands.",
                        "inputSchema": {"type": "object", "properties": {}}
                    }
                ]
            })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "tools/call needs `name`".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match name {
                    "execute" => {
                        let cmd = args.get("command").and_then(Value::as_str).ok_or((INVALID_PARAMS, "execute needs `command`".to_string()))?;
                        let p = args.get("params").cloned().unwrap_or(json!({}));
                        match self.session.execute(cmd, p) {
                            Ok(v) => Ok(json!({"content": [{"type": "text", "text": v.to_string()}]})),
                            Err(e) => Ok(json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true})),
                        }
                    }
                    "list_commands" => match self.session.execute("engine.list", json!({})) {
                        Ok(v) => Ok(json!({"content": [{"type": "text", "text": v.to_string()}]})),
                        Err(e) => Ok(json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true})),
                    },
                    other => Ok(json!({"content": [{"type": "text", "text": format!("unknown tool '{other}'")}], "isError": true})),
                }
            }
            "resources/list" => Ok(json!({
                "resources": [
                    {"uri": "reportcraft://model", "name": "Semantic model summary", "mimeType": "application/json"},
                    {"uri": "reportcraft://commands", "name": "Engine commands", "mimeType": "application/json"}
                ]
            })),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).ok_or((INVALID_PARAMS, "resources/read needs `uri`".to_string()))?;
                let res = match uri {
                    "reportcraft://model" => self.session.execute("project.summary", json!({})),
                    "reportcraft://commands" => self.session.execute("engine.list", json!({})),
                    _ => return Err((INVALID_PARAMS, format!("unknown resource '{uri}'"))),
                };
                match res {
                    Ok(v) => Ok(json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": v.to_string()}]})),
                    Err(e) => Err((INTERNAL_ERROR, e.to_string())),
                }
            }
            _ => Err((METHOD_NOT_FOUND, format!("unknown method '{method}'"))),
        }
    }
}

fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_initialize_and_execute() {
        let mut s = Server::new();
        let reply = s.handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap();
        let v: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(v["result"]["serverInfo"]["name"], "reportcraft");

        let call = s.handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_commands","arguments":{}}}"#).unwrap();
        assert!(call.contains("table.import_csv"));
    }
}
