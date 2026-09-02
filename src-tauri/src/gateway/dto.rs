use serde_json::Value;

pub fn model(body: &Value) -> Option<&str> {
    body.get("model").and_then(Value::as_str)
}

pub fn reasoning_effort(body: &Value) -> Option<String> {
    let effort = body
        .get("reasoning_effort")
        .filter(|value| !value.is_null())
        .or_else(|| body.get("reasoning").and_then(|value| value.get("effort")))?;
    effort.as_str().map(str::to_owned).or_else(|| {
        if effort.is_null() {
            None
        } else {
            Some(effort.to_string())
        }
    })
}

fn usage_object(body: &Value) -> Option<&Value> {
    [
        body.get("usage"),
        body.get("response").and_then(|value| value.get("usage")),
        body.get("message").and_then(|value| value.get("usage")),
    ]
    .into_iter()
    .flatten()
    .find(|value| value.is_object())
}

fn usage_fields(body: &Value) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>) {
    let Some(u) = usage_object(body) else {
        return (None, None, None, None);
    };
    let standard_input = u
        .get("prompt_tokens")
        .or_else(|| u.get("input_tokens"))
        .and_then(Value::as_i64);
    let output = u
        .get("completion_tokens")
        .or_else(|| u.get("output_tokens"))
        .and_then(Value::as_i64);
    let total = u.get("total_tokens").and_then(Value::as_i64);
    let standard_cached = u
        .get("cached_tokens")
        .or_else(|| u.pointer("/prompt_tokens_details/cached_tokens"))
        .or_else(|| u.pointer("/input_tokens_details/cached_tokens"))
        .and_then(Value::as_i64);

    // Anthropic reports uncached input, cache creation, and cache reads separately.
    // Normalize input to the full prompt size so cache-rate calculations match OpenAI.
    let anthropic_usage = u.get("cache_read_input_tokens").is_some()
        || u.get("cache_creation_input_tokens").is_some()
        || u.get("cache_creation").is_some();
    if !anthropic_usage {
        return (standard_input, output, total, standard_cached);
    }

    let uncached_input = u.get("input_tokens").and_then(Value::as_i64);
    let cache_read = u.get("cache_read_input_tokens").and_then(Value::as_i64);
    let cache_creation = u
        .get("cache_creation_input_tokens")
        .and_then(Value::as_i64)
        .or_else(|| {
            u.get("cache_creation")
                .and_then(Value::as_object)
                .map(|creation| {
                    creation
                        .iter()
                        .filter(|(key, _)| key.ends_with("_input_tokens"))
                        .filter_map(|(_, value)| value.as_i64())
                        .sum()
                })
        });
    let input = (uncached_input.is_some() || cache_creation.is_some() || cache_read.is_some())
        .then(|| {
            uncached_input.unwrap_or(0) + cache_creation.unwrap_or(0) + cache_read.unwrap_or(0)
        });
    (input, output, total, cache_read)
}

fn with_estimated_total(
    (input, output, total, cached): (Option<i64>, Option<i64>, Option<i64>, Option<i64>),
) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>) {
    let total = total.or_else(|| {
        (input.is_some() || output.is_some()).then(|| input.unwrap_or(0) + output.unwrap_or(0))
    });
    (input, output, total, cached)
}

pub fn usage(body: &Value) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>) {
    with_estimated_total(usage_fields(body))
}

pub fn usage_from_sse(bytes: &[u8]) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>) {
    let text = String::from_utf8_lossy(bytes);
    let mut result = (None, None, None, None);
    for line in text.lines() {
        let line = line.trim();
        let data = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
        if data == "[DONE]" {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(data) {
            merge_usage(&mut result, usage_fields(&value));
        }
    }
    with_estimated_total(result)
}

pub fn stream_outcome(bytes: &[u8]) -> Option<bool> {
    let text = String::from_utf8_lossy(bytes);
    let mut event_name = None;
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            event_name = None;
            continue;
        }
        if let Some(event) = line.strip_prefix("event:").map(str::trim) {
            event_name = Some(event);
            continue;
        }
        let data = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
        if data == "[DONE]" {
            return Some(true);
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(event) = event_name {
            match event {
                "response.completed" | "message_stop" => return Some(true),
                "response.failed" | "response.incomplete" | "response.cancelled" | "error" => {
                    return Some(false)
                }
                _ => {}
            }
        }
        match value.get("type").and_then(Value::as_str) {
            Some("response.completed") | Some("message_stop") => return Some(true),
            Some("response.failed")
            | Some("response.incomplete")
            | Some("response.cancelled")
            | Some("error") => return Some(false),
            _ => {}
        }
    }
    None
}

fn merge_usage(
    target: &mut (Option<i64>, Option<i64>, Option<i64>, Option<i64>),
    current: (Option<i64>, Option<i64>, Option<i64>, Option<i64>),
) {
    if current.0.is_some() {
        target.0 = current.0;
    }
    if current.1.is_some() {
        target.1 = current.1;
    }
    if current.2.is_some() {
        target.2 = current.2;
    }
    if current.3.is_some() {
        target.3 = current.3;
    }
}
