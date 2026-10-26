//! JSONL framing: one complete JSON object per record, LF-terminated
//! (an optional preceding CR is stripped, so CRLF input is accepted).
//! Never splits on Unicode separators. Malformed lines are skipped and
//! counted by the client — a bad record must not kill the session.

/// Encode one command: canonical JSON object + trailing LF.
pub fn encode(cmd_type: &str, id: &str, fields: &[(&str, serde_json::Value)]) -> Vec<u8> {
    let mut map = serde_json::Map::with_capacity(fields.len() + 2);
    map.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    map.insert(
        "type".to_string(),
        serde_json::Value::String(cmd_type.to_string()),
    );
    for (k, v) in fields {
        map.insert((*k).to_string(), v.clone());
    }
    let mut bytes = serde_json::to_vec(&map).expect("command serializes");
    bytes.push(b'\n');
    bytes
}

/// Strip one trailing CR so LF and CRLF input are both accepted.
pub fn strip_terminator(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

/// A classified record.
#[derive(Clone, Debug)]
pub enum Record {
    /// `{type:"response", id?, ...}` — correlated by id when present.
    Response {
        id: Option<String>,
        value: serde_json::Value,
    },
    /// `{type:"message_update", assistantMessageEvent:{type:"text_delta", delta}}`
    TextDelta(String),
    /// `{type:"agent_settled", aborted?}`
    Settled { aborted: bool },
    /// Tool activity worth showing while a run is live (announced calls,
    /// starts, completions). Summarized at classification time; the raw
    /// record stays available as `Event` for future consumers.
    ToolNote(String),
    /// Anything else (session events, future types).
    Event {
        etype: String,
        value: serde_json::Value,
    },
}

/// Parse + classify one line. `Err` = malformed (skip + count).
pub fn decode(line: &str) -> Result<Record, String> {
    let line = strip_terminator(line.trim_end_matches('\n'));
    if line.trim().is_empty() {
        return Err("blank line".to_string());
    }
    let v: serde_json::Value = serde_json::from_str(line).map_err(|e| format!("bad json: {e}"))?;
    let etype = v
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    match etype.as_str() {
        "response" => Ok(Record::Response {
            id: v.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()),
            value: v,
        }),
        "message_update" => {
            let ev = v.get("assistantMessageEvent");
            let kind = ev.and_then(|e| e.get("type")).and_then(|t| t.as_str());
            match kind {
                Some("toolcall_start") => {
                    let name = ev
                        .and_then(|e| e.get("toolName"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("?");
                    Ok(Record::ToolNote(format!("tool {name} …")))
                }
                _ => {
                    let delta = match kind {
                        Some("text_delta") => ev
                            .and_then(|e| e.get("delta"))
                            .and_then(|d| d.as_str())
                            .unwrap_or(""),
                        _ => "",
                    }
                    .to_string();
                    Ok(Record::TextDelta(delta))
                }
            }
        }
        "tool_execution_start" => {
            let name = v.get("toolName").and_then(|n| n.as_str()).unwrap_or("?");
            let args = v.get("args").and_then(|a| a.as_str()).unwrap_or("");
            let head: String = args.chars().take(120).collect();
            let more = if args.chars().count() > 120 {
                "…"
            } else {
                ""
            };
            let what = if head.is_empty() {
                "started".to_string()
            } else {
                format!("{head}{more}")
            };
            Ok(Record::ToolNote(format!("tool {name} {what}")))
        }
        "tool_execution_end" => {
            let name = v.get("toolName").and_then(|n| n.as_str()).unwrap_or("?");
            let failed = v.get("isError").and_then(|b| b.as_bool()).unwrap_or(false);
            let ms = v
                .get("durationMs")
                .and_then(|d| d.as_i64())
                .map(|d| format!(" ({d}ms)"))
                .unwrap_or_default();
            Ok(Record::ToolNote(format!(
                "tool {name} {}{ms}",
                if failed { "failed" } else { "done" }
            )))
        }
        "agent_settled" => Ok(Record::Settled {
            aborted: v.get("aborted").and_then(|a| a.as_bool()).unwrap_or(false),
        }),
        _ => Ok(Record::Event { etype, value: v }),
    }
}

pub fn is_success_response(v: &serde_json::Value) -> Result<&serde_json::Value, String> {
    if v.get("success").and_then(|s| s.as_bool()) == Some(false) {
        let err = v
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown error");
        return Err(err.to_string());
    }
    Ok(v.get("data").unwrap_or(&serde_json::Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let b = encode("get_state", "abc", &[]);
        assert!(b.ends_with(b"\n"));
        let s = String::from_utf8(b).unwrap();
        assert!(s.contains("\"type\":\"get_state\""));
        assert!(s.contains("\"id\":\"abc\""));
    }

    #[test]
    fn crlf_accepted() {
        let r = decode("{\"type\":\"agent_settled\",\"aborted\":true}\r\n").unwrap();
        assert!(matches!(r, Record::Settled { aborted: true }));
    }

    #[test]
    fn text_delta_extracted() {
        let r = decode("{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"delta\":\"hi\"}}").unwrap();
        match r {
            Record::TextDelta(d) => assert_eq!(d, "hi"),
            other => panic!("wrong record: {other:?}"),
        }
    }

    #[test]
    fn malformed_rejected() {
        assert!(decode("not json\n").is_err());
        assert!(decode("\n").is_err());
        // Non-delta message_update yields empty delta, not an error.
        let r =
            decode("{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"other\"}}")
                .unwrap();
        assert!(matches!(r, Record::TextDelta(_)));
    }

    #[test]
    fn response_classified() {
        let r = decode("{\"type\":\"response\",\"id\":\"7\",\"command\":\"prompt\",\"data\":{}}")
            .unwrap();
        match r {
            Record::Response { id, .. } => assert_eq!(id.as_deref(), Some("7")),
            other => panic!("wrong record: {other:?}"),
        }
    }

    #[test]
    fn success_gate() {
        let bad: serde_json::Value =
            serde_json::from_str("{\"success\":false,\"error\":\"nope\"}").unwrap();
        assert!(is_success_response(&bad).is_err());
        let ok: serde_json::Value =
            serde_json::from_str("{\"success\":true,\"data\":{\"a\":1}}").unwrap();
        assert!(is_success_response(&ok).unwrap().get("a").is_some());
    }
}
