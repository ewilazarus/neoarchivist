//! `neoarchivist contract check request|reply`: checks a message between the main agent and
//! `neoarchivist:agent` against their contract. A message is one JSON object, alone or in a
//! ```json fence. Unknown fields are problems too, so a misspelt field fails instead of
//! being dropped.
//!
//! Requests:
//!   {"kind": "ask", "question": "…", "why": "…"}                    why is optional
//!   {"kind": "record",                                              every list optional,
//!    "changed":  ["…"],                                             at least one not empty
//!    "decided":  [{"what": "…", "why": "…", "rejected": "…"}],      rejected is optional
//!    "dropped":  [{"what": "…", "why": "…"}],
//!    "open":     [{"todo": "…", "done_when": "…"}],
//!    "finished": ["…"], "reopened": ["…"],
//!    "wrong":    [{"note": "…", "issue": "…"}],
//!    "commits":  "…"}                                               optional
//!   {"kind": "answers", "answers": ["…"]}                           to an agent that asked
//!
//! Replies, whose status names follow the A2A task states:
//!   {"status": "completed",      "answer": "…", "sources": ["…"], "disagreements": ["…"],
//!                                "written": [{"path": "…", "change": "…"}], "reason": "…"}
//!   {"status": "input-required", "questions": [{"q": "…", "options": ["…"]}], "written": […]}
//!   {"status": "failed",         "reason": "…"}
//! A completed reply has at least one of answer, written or reason.

use serde_json::{Map, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Request,
    Reply,
}

/// The message without surrounding blank lines or a ```json fence around the whole of it.
pub fn unwrap(message: &str) -> String {
    let lines: Vec<&str> = message.lines().collect();
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    let (Some(mut a), Some(mut b)) = (first, last) else {
        return String::new();
    };
    if b > a && matches!(lines[a].trim(), "```" | "```json") && lines[b].trim() == "```" {
        a += 1;
        b -= 1;
    }
    lines[a..=b].join("\n")
}

/// The message as one JSON object, or the problem.
pub fn parse(message: &str) -> Result<Map<String, Value>, String> {
    let body = unwrap(message);
    let mut values = serde_json::Deserializer::from_str(&body).into_iter::<Value>();
    let problem =
        || "The message isn't one JSON object: send only the object, alone or in a ```json fence.".to_string();
    let first = match values.next() {
        Some(Ok(Value::Object(map))) => map,
        _ => return Err(problem()),
    };
    if values.next().is_some() {
        return Err(problem());
    }
    Ok(first)
}

/// Every problem with the message, in order. None means it fits.
pub fn check(side: Side, message: &str) -> Vec<String> {
    let object = match parse(message) {
        Ok(o) => o,
        Err(p) => return vec![p],
    };
    let mut c = Checker { problems: Vec::new() };
    match side {
        Side::Request => c.request(&object),
        Side::Reply => c.reply(&object),
    }
    c.problems
}

fn text(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::String(s)) if !s.trim().is_empty())
}

fn texts(v: &Value) -> bool {
    matches!(v, Value::Array(a) if a.iter().all(|x| text(Some(x))))
}

fn non_empty_list(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Array(a)) if !a.is_empty())
}

struct Checker {
    problems: Vec<String>,
}

impl Checker {
    fn push(&mut self, p: impl Into<String>) {
        self.problems.push(p.into());
    }

    fn only(&mut self, o: &Map<String, Value>, allowed: &[&str], place: &str) {
        let mut keys: Vec<&String> = o.keys().filter(|k| !allowed.contains(&k.as_str())).collect();
        keys.sort();
        for k in keys {
            self.push(format!("{place}has an unknown field \"{k}\"."));
        }
    }

    fn need(&mut self, o: &Map<String, Value>, key: &str, place: &str) {
        if !text(o.get(key)) {
            self.push(format!("{place}needs \"{key}\", as text."));
        }
    }

    fn maybe(&mut self, o: &Map<String, Value>, key: &str, place: &str) {
        if o.contains_key(key) && !text(o.get(key)) {
            self.push(format!("{place}\"{key}\" must be text."));
        }
    }

    fn list(&mut self, o: &Map<String, Value>, key: &str) {
        if let Some(v) = o.get(key) {
            if !texts(v) {
                self.push(format!("\"{key}\" must be a list of text."));
            }
        }
    }

    fn items(&mut self, o: &Map<String, Value>, key: &str, required: &[&str], optional: &[&str]) {
        let Some(v) = o.get(key) else { return };
        let Value::Array(items) = v else {
            self.push(format!("\"{key}\" must be a list."));
            return;
        };
        let allowed: Vec<&str> = required.iter().chain(optional).copied().collect();
        for (i, item) in items.iter().enumerate() {
            let place = format!("\"{key}\"[{i}] ");
            let Value::Object(item) = item else {
                self.push(format!("\"{key}\"[{i}] must be an object."));
                continue;
            };
            self.only(item, &allowed, &place);
            for r in required {
                self.need(item, r, &place);
            }
            for opt in optional {
                self.maybe(item, opt, &place);
            }
        }
    }

    fn request(&mut self, o: &Map<String, Value>) {
        match o.get("kind").and_then(Value::as_str) {
            Some("ask") => {
                self.only(o, &["kind", "question", "why"], "An ask ");
                self.need(o, "question", "An ask ");
                self.maybe(o, "why", "An ask ");
            }
            Some("answers") => {
                self.only(o, &["kind", "answers"], "An answers request ");
                if !(o.get("answers").is_some_and(texts) && non_empty_list(o.get("answers"))) {
                    self.push("\"answers\" must be a non-empty list of text.");
                }
            }
            Some("record") => {
                self.only(
                    o,
                    &[
                        "kind", "changed", "decided", "dropped", "open", "finished", "reopened", "wrong", "commits",
                    ],
                    "A record ",
                );
                self.list(o, "changed");
                self.list(o, "finished");
                self.list(o, "reopened");
                self.items(o, "decided", &["what", "why"], &["rejected"]);
                self.items(o, "dropped", &["what", "why"], &[]);
                self.items(o, "open", &["todo", "done_when"], &[]);
                self.items(o, "wrong", &["note", "issue"], &[]);
                self.maybe(o, "commits", "A record ");
                let lists = ["changed", "decided", "dropped", "open", "finished", "reopened", "wrong"];
                if !lists.iter().any(|k| non_empty_list(o.get(*k))) {
                    self.push("A record needs at least one non-empty list: changed, decided, dropped, open, finished, reopened or wrong.");
                }
            }
            _ => self.push("\"kind\" must be \"ask\", \"record\" or \"answers\"."),
        }
    }

    fn reply(&mut self, o: &Map<String, Value>) {
        self.only(
            o,
            &[
                "status",
                "answer",
                "sources",
                "disagreements",
                "written",
                "questions",
                "reason",
            ],
            "The reply ",
        );
        self.list(o, "sources");
        self.list(o, "disagreements");
        self.items(o, "written", &["path", "change"], &[]);
        self.maybe(o, "answer", "The reply ");
        self.maybe(o, "reason", "The reply ");
        match o.get("questions") {
            Some(Value::Array(qs)) => {
                for (i, q) in qs.iter().enumerate() {
                    let place = format!("\"questions\"[{i}] ");
                    let Value::Object(q) = q else {
                        self.push(format!("\"questions\"[{i}] must be an object."));
                        continue;
                    };
                    self.only(q, &["q", "options"], &place);
                    self.need(q, "q", &place);
                    if q.get("options").is_some_and(|v| !texts(v)) {
                        self.push(format!("\"questions\"[{i}] \"options\" must be a list of text."));
                    }
                }
            }
            Some(_) => self.push("\"questions\" must be a list."),
            None => {}
        }
        match o.get("status").and_then(Value::as_str) {
            Some("completed") => {
                if !(text(o.get("answer")) || non_empty_list(o.get("written")) || text(o.get("reason"))) {
                    self.push(
                        "A completed reply needs an answer, what was written, or a reason there was nothing to write.",
                    );
                }
            }
            Some("input-required") => {
                if !non_empty_list(o.get("questions")) {
                    self.push("An input-required reply needs its questions.");
                }
            }
            Some("failed") => self.need(o, "reason", "A failed reply "),
            _ => self.push("\"status\" must be \"completed\", \"input-required\" or \"failed\"."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenced_and_bare_messages_parse() {
        assert!(
            check(
                Side::Request,
                "```json\n{\"kind\": \"ask\", \"question\": \"x\"}\n```\n"
            )
            .is_empty()
        );
        assert!(check(Side::Request, "  \n{\"kind\": \"ask\", \"question\": \"x\"}\n\n").is_empty());
    }

    #[test]
    fn two_objects_are_refused() {
        assert_eq!(
            check(
                Side::Request,
                r#"{"kind":"ask","question":"a"} {"kind":"ask","question":"b"}"#
            )
            .len(),
            1
        );
    }

    #[test]
    fn problems_come_in_field_order() {
        let p = check(
            Side::Request,
            r#"{"kind":"record","dicided":[],"decided":[{"what":"x"}],"open":[{"todo":"y"}]}"#,
        );
        assert_eq!(
            p,
            vec![
                "A record has an unknown field \"dicided\".",
                "\"decided\"[0] needs \"why\", as text.",
                "\"open\"[0] needs \"done_when\", as text."
            ]
        );
    }
}
