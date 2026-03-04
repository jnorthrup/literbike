use literbike::env_facade_parity::{
    evaluate_modelmux_mvp_quota_inventory, format_modelmux_mvp_lifecycle_line,
    format_modelmux_mvp_quota_selection_line, format_quota_drainer_dry_run_line,
    run_modelmux_mvp_lifecycle_with_options, run_modelmux_quota_drainer_dry_run,
    run_modelmux_quota_drainer_dry_run_with_options, GenericApiModelsProbe,
    GenericApiModelsProbeCache, GenericApiModelsProbeClassification, MockQuotaInventoryRecord,
    PragmaticUnifiedPortConfig, QuotaDrainerDryRunOptions, QuotaInventoryAdapter,
    StaticMockQuotaInventoryAdapter,
};
use literbike::model_serving_taxonomy::ProviderFamily;
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

#[derive(Debug, Default)]
struct CliArgs {
    model_ref: Option<String>,
    env_file: Option<String>,
    env_overrides: Vec<(String, String)>,
    mock_quota_specs: Vec<String>,
    agent_name: Option<String>,
    port: Option<u16>,
    probe_mode: Option<String>,
    probe_timeout_secs: u64,
    quota_min_req: Option<u64>,
    quota_min_tok: Option<u64>,
    ignore_process_env: bool,
}

fn usage() -> &'static str {
    "Usage: modelmux_mvp_lifecycle --model <ref> [--ignore-process-env] [--env-file <path>] [--env KEY=VAL] [--mock-quota <slot::model[;req=N][;tok=N][;free|paid][;selector=TAG]>] [--quota-min-req <n>] [--quota-min-tok <n>] [--agent-name <name>] [--port <n>] [--probe curl] [--probe-timeout-secs <n>]\n\
Examples:\n\
  modelmux_mvp_lifecycle --model /free/moonshotai/kimi-k2 --ignore-process-env --env OPENAI_API_KEY=...\n\
  modelmux_mvp_lifecycle --model '/{api.anthropic.com:443,tools,https}/claude-opus-4-6' --env ANTHROPIC_AUTH_TOKEN=... \n\
  modelmux_mvp_lifecycle --model /free/moonshotai/kimi-k2 --ignore-process-env --env-file .env --agent-name agent8888 --port 8888 --probe curl\n\
  modelmux_mvp_lifecycle --model /free/moonshotai/kimi-k2 --ignore-process-env --env OPENAI_API_KEY=... --mock-quota free-kimi::/free/moonshotai/kimi-k2;req=25;tok=20000\n\
  modelmux_mvp_lifecycle --model /free/moonshotai/kimi-k2 --ignore-process-env --env OPENAI_API_KEY=... --mock-quota free::/free/moonshotai/kimi-k2;req=2;tok=2000 --mock-quota paid::moonshotai/kimi-k2;req=9;tok=9000 --quota-min-req 3 --quota-min-tok 3000"
}

fn main() {
    let args = match parse_args(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    };

    let model_ref = match args.model_ref.clone() {
        Some(v) => v,
        None => {
            eprintln!("missing required --model <ref>");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    };

    let mut merged_env = if args.ignore_process_env {
        BTreeMap::new()
    } else {
        collect_process_env()
    };
    if let Some(path) = args.env_file.as_deref() {
        if let Err(err) = merge_env_file(&mut merged_env, path) {
            eprintln!("failed to read env file `{path}`: {err}");
            std::process::exit(2);
        }
    }
    for (k, v) in &args.env_overrides {
        merged_env.insert(k.clone(), v.clone());
    }
    let env_pairs: Vec<(String, String)> = merged_env.into_iter().collect();

    let cfg = if args.agent_name.is_some() || args.port.is_some() {
        Some(PragmaticUnifiedPortConfig {
            agent_name: args
                .agent_name
                .clone()
                .unwrap_or_else(|| "unified-port".to_string()),
            unified_port: args.port.unwrap_or(8888),
        })
    } else {
        None
    };

    let mut probe_cache = GenericApiModelsProbeCache::default();
    let curl_probe = if args
        .probe_mode
        .as_deref()
        .map(|m| m.eq_ignore_ascii_case("curl"))
        .unwrap_or(false)
    {
        Some(CurlModelsProbe {
            timeout_secs: if args.probe_timeout_secs == 0 {
                3
            } else {
                args.probe_timeout_secs
            },
        })
    } else {
        None
    };

    let lifecycle = match run_modelmux_mvp_lifecycle_with_options(
        env_pairs,
        &model_ref,
        cfg.as_ref(),
        curl_probe.as_ref().map(|p| p as &dyn GenericApiModelsProbe),
        Some(&mut probe_cache),
    ) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("model ref parse/lifecycle error: {err:?}");
            std::process::exit(1);
        }
    };

    println!("{}", format_modelmux_mvp_lifecycle_line(&lifecycle));
    println!("route_line={}", {
        let route = &lifecycle.route;
        let widened = route
            .widened_models
            .iter()
            .map(|w| format!("{}:{}", w.boundary, w.model))
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "agent={};port={};route_key={};model={};selected_key={};widened={widened}",
            route.agent_name,
            route.unified_port,
            route.route_key,
            route.upstream_model_id,
            lifecycle
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str())
                .unwrap_or("")
        )
    });
    if !args.mock_quota_specs.is_empty() {
        let records = match args
            .mock_quota_specs
            .iter()
            .map(|s| parse_mock_quota_spec(s))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(v) => v,
            Err(msg) => {
                eprintln!("invalid --mock-quota: {msg}");
                std::process::exit(2);
            }
        };
        let adapter = StaticMockQuotaInventoryAdapter { records };
        let inventory = match adapter.load_quota_inventory() {
            Ok(v) => v,
            Err(err) => {
                eprintln!("failed to load mock quota inventory: {}", err.message);
                std::process::exit(1);
            }
        };
        let selection = evaluate_modelmux_mvp_quota_inventory(&lifecycle, &inventory);
        println!("{}", format_modelmux_mvp_quota_selection_line(&selection));
        let dry_run = if args.quota_min_req.is_some() || args.quota_min_tok.is_some() {
            let mut opts = QuotaDrainerDryRunOptions::default();
            if let Some(v) = args.quota_min_req {
                opts.min_remaining_requests = v;
            }
            if let Some(v) = args.quota_min_tok {
                opts.min_remaining_tokens = v;
            }
            run_modelmux_quota_drainer_dry_run_with_options(&lifecycle, &inventory, &opts)
        } else {
            run_modelmux_quota_drainer_dry_run(&lifecycle, &inventory)
        };
        println!("{}", format_quota_drainer_dry_run_line(&dry_run));
    }
    if !lifecycle.readiness.ready {
        std::process::exit(3);
    }
}

fn parse_args(argv: Vec<String>) -> Result<CliArgs, String> {
    let mut out = CliArgs {
        probe_timeout_secs: 3,
        ..CliArgs::default()
    };
    let mut i = 0usize;
    while i < argv.len() {
        let cur = &argv[i];
        match cur.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            "--model" => {
                i += 1;
                out.model_ref = Some(
                    argv.get(i)
                        .cloned()
                        .ok_or_else(|| "missing value for --model".to_string())?,
                );
            }
            "--env-file" => {
                i += 1;
                out.env_file = Some(
                    argv.get(i)
                        .cloned()
                        .ok_or_else(|| "missing value for --env-file".to_string())?,
                );
            }
            "--ignore-process-env" => {
                out.ignore_process_env = true;
            }
            "--env" => {
                i += 1;
                let pair = argv
                    .get(i)
                    .ok_or_else(|| "missing value for --env".to_string())?;
                out.env_overrides.push(parse_env_assignment(pair)?);
            }
            "--mock-quota" => {
                i += 1;
                out.mock_quota_specs.push(
                    argv.get(i)
                        .cloned()
                        .ok_or_else(|| "missing value for --mock-quota".to_string())?,
                );
            }
            "--quota-min-req" => {
                i += 1;
                let v = argv
                    .get(i)
                    .ok_or_else(|| "missing value for --quota-min-req".to_string())?;
                out.quota_min_req = Some(
                    v.parse::<u64>()
                        .map_err(|_| format!("invalid --quota-min-req `{v}`"))?,
                );
            }
            "--quota-min-tok" => {
                i += 1;
                let v = argv
                    .get(i)
                    .ok_or_else(|| "missing value for --quota-min-tok".to_string())?;
                out.quota_min_tok = Some(
                    v.parse::<u64>()
                        .map_err(|_| format!("invalid --quota-min-tok `{v}`"))?,
                );
            }
            "--agent-name" => {
                i += 1;
                out.agent_name = Some(
                    argv.get(i)
                        .cloned()
                        .ok_or_else(|| "missing value for --agent-name".to_string())?,
                );
            }
            "--port" => {
                i += 1;
                let v = argv
                    .get(i)
                    .ok_or_else(|| "missing value for --port".to_string())?;
                out.port = Some(
                    v.parse::<u16>()
                        .map_err(|_| format!("invalid --port `{v}`"))?,
                );
            }
            "--probe" => {
                i += 1;
                out.probe_mode = Some(
                    argv.get(i)
                        .cloned()
                        .ok_or_else(|| "missing value for --probe".to_string())?,
                );
            }
            "--probe-timeout-secs" => {
                i += 1;
                let v = argv
                    .get(i)
                    .ok_or_else(|| "missing value for --probe-timeout-secs".to_string())?;
                out.probe_timeout_secs = v
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --probe-timeout-secs `{v}`"))?;
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown flag `{other}`"));
            }
            positional => {
                if out.model_ref.is_none() {
                    out.model_ref = Some(positional.to_string());
                } else {
                    return Err(format!("unexpected positional argument `{positional}`"));
                }
            }
        }
        i += 1;
    }
    Ok(out)
}

fn parse_env_assignment(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("expected KEY=VAL, got `{s}`"))?;
    let key = k.trim();
    if key.is_empty() {
        return Err(format!("empty key in `{s}`"));
    }
    Ok((key.to_string(), strip_matching_quotes(v.trim()).to_string()))
}

fn parse_mock_quota_spec(s: &str) -> Result<MockQuotaInventoryRecord, String> {
    let (slot_id_raw, remainder) = s
        .split_once("::")
        .ok_or_else(|| format!("expected <slot_id>::<model_ref_or_id>[;flags], got `{s}`"))?;
    let slot_id = slot_id_raw.trim();
    if slot_id.is_empty() {
        return Err(format!("empty slot id in `{s}`"));
    }

    let mut parts = remainder.split(';');
    let model_ref_or_id = parts
        .next()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("missing model ref/id in `{s}`"))?
        .to_string();

    let mut record = MockQuotaInventoryRecord {
        slot_id: slot_id.to_string(),
        model_ref_or_id,
        base_url: None,
        enabled: true,
        healthy: true,
        free: false,
        selectors: Vec::new(),
        remaining_requests: None,
        remaining_tokens: None,
        metadata: BTreeMap::new(),
        notes: Vec::new(),
    };

    for raw in parts {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        if token.eq_ignore_ascii_case("free") {
            record.free = true;
            continue;
        }
        if token.eq_ignore_ascii_case("paid") {
            record.free = false;
            continue;
        }
        if token.eq_ignore_ascii_case("disabled") {
            record.enabled = false;
            continue;
        }
        if token.eq_ignore_ascii_case("unhealthy") {
            record.healthy = false;
            continue;
        }
        if let Some(v) = token.strip_prefix("req=") {
            record.remaining_requests = Some(
                v.trim()
                    .parse::<u64>()
                    .map_err(|_| format!("invalid req in `{s}`"))?,
            );
            continue;
        }
        if let Some(v) = token.strip_prefix("tok=") {
            record.remaining_tokens = Some(
                v.trim()
                    .parse::<u64>()
                    .map_err(|_| format!("invalid tok in `{s}`"))?,
            );
            continue;
        }
        if let Some(v) = token.strip_prefix("base=") {
            let v = v.trim();
            if !v.is_empty() {
                record.base_url = Some(v.to_string());
            }
            continue;
        }
        if let Some(v) = token.strip_prefix("selector=") {
            let v = v.trim();
            if !v.is_empty() {
                record.selectors.push(v.to_string());
            }
            continue;
        }
        if let Some(v) = token.strip_prefix("sel=") {
            let v = v.trim();
            if !v.is_empty() {
                record.selectors.push(v.to_string());
            }
            continue;
        }
        if let Some(v) = token.strip_prefix("note=") {
            let v = v.trim();
            if !v.is_empty() {
                record.notes.push(v.to_string());
            }
            continue;
        }
        if let Some((k, v)) = token.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            if !key.is_empty() && !val.is_empty() {
                record.metadata.insert(key.to_string(), val.to_string());
                continue;
            }
        }
        return Err(format!("unrecognized mock quota token `{token}` in `{s}`"));
    }

    Ok(record)
}

fn collect_process_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

fn merge_env_file(out: &mut BTreeMap<String, String>, path: &str) -> Result<(), String> {
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    for raw_line in data.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let (k, v) = parse_env_assignment(line)?;
        out.insert(k, v);
    }
    Ok(())
}

fn strip_matching_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        let first = bytes[0];
        let last = bytes[s.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

struct CurlModelsProbe {
    timeout_secs: u64,
}

impl GenericApiModelsProbe for CurlModelsProbe {
    fn probe_models_capability(
        &self,
        _base_url: &str,
        candidate_urls: &[String],
    ) -> Option<GenericApiModelsProbeClassification> {
        for url in candidate_urls {
            let output = Command::new("curl")
                .arg("-sS")
                .arg("-m")
                .arg(self.timeout_secs.to_string())
                .arg("-o")
                .arg("-")
                .arg("-w")
                .arg("\n__CURL_STATUS__:%{http_code}")
                .arg("-H")
                .arg("accept: application/json")
                .arg(url)
                .output()
                .ok()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let (body, status) = parse_curl_status_output(&stdout);
            let status = status.unwrap_or(0);

            if matches!(status, 200 | 401 | 403) {
                let body_lc = body.to_ascii_lowercase();
                let family_hint = if body_lc.contains("anthropic") || body_lc.contains("claude") {
                    Some(ProviderFamily::AnthropicCompatible)
                } else if body_lc.contains("gemini") || body_lc.contains("google") {
                    Some(ProviderFamily::GeminiNative)
                } else {
                    Some(ProviderFamily::OpenAiCompatible)
                };

                return Some(GenericApiModelsProbeClassification {
                    api_kind: literbike::env_facade_parity::ApiKind::ModelProvider,
                    family_hint,
                    confidence: if status == 200 { 92 } else { 78 },
                    reason: format!("curl models probe HTTP {status}"),
                    matched_probe_url: Some(url.clone()),
                });
            }
        }
        None
    }
}

fn parse_curl_status_output(stdout: &str) -> (String, Option<u16>) {
    let marker = "\n__CURL_STATUS__:";
    if let Some(idx) = stdout.rfind(marker) {
        let body = stdout[..idx].to_string();
        let code_str = stdout[(idx + marker.len())..].trim();
        let code = code_str.parse::<u16>().ok();
        (body, code)
    } else {
        (stdout.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mock_quota_spec_with_flags() {
        let rec = parse_mock_quota_spec(
            "free-kimi::/free/moonshotai/kimi-k2;req=25;tok=20000;selector=quota-dsel-free;note=vm",
        )
        .expect("record");

        assert_eq!(rec.slot_id, "free-kimi");
        assert_eq!(rec.model_ref_or_id, "/free/moonshotai/kimi-k2");
        assert_eq!(rec.remaining_requests, Some(25));
        assert_eq!(rec.remaining_tokens, Some(20_000));
        assert!(rec.selectors.iter().any(|s| s == "quota-dsel-free"));
        assert!(rec.notes.iter().any(|n| n == "vm"));
    }

    #[test]
    fn parse_args_collects_repeated_mock_quota_flags() {
        let args = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--mock-quota".to_string(),
            "a::/free/moonshotai/kimi-k2;free".to_string(),
            "--mock-quota".to_string(),
            "b::moonshotai/kimi-k2;paid".to_string(),
            "--quota-min-req".to_string(),
            "3".to_string(),
            "--quota-min-tok".to_string(),
            "3000".to_string(),
        ])
        .expect("args");

        assert_eq!(args.model_ref.as_deref(), Some("/free/moonshotai/kimi-k2"));
        assert_eq!(args.mock_quota_specs.len(), 2);
        assert_eq!(args.mock_quota_specs[0], "a::/free/moonshotai/kimi-k2;free");
        assert_eq!(args.mock_quota_specs[1], "b::moonshotai/kimi-k2;paid");
        assert_eq!(args.quota_min_req, Some(3));
        assert_eq!(args.quota_min_tok, Some(3000));
    }

    #[test]
    fn parse_args_rejects_invalid_quota_minima_values() {
        let err_req = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--quota-min-req".to_string(),
            "abc".to_string(),
        ])
        .expect_err("expected invalid req error");
        assert!(err_req.contains("invalid --quota-min-req"));

        let err_tok = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--quota-min-tok".to_string(),
            "nope".to_string(),
        ])
        .expect_err("expected invalid tok error");
        assert!(err_tok.contains("invalid --quota-min-tok"));
    }

    #[test]
    fn parse_args_rejects_missing_quota_minima_values() {
        let err_req = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--quota-min-req".to_string(),
        ])
        .expect_err("expected missing req value error");
        assert!(err_req.contains("missing value for --quota-min-req"));

        let err_tok = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--quota-min-tok".to_string(),
        ])
        .expect_err("expected missing tok value error");
        assert!(err_tok.contains("missing value for --quota-min-tok"));
    }

    #[test]
    fn parse_args_rejects_missing_mock_quota_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--mock-quota".to_string(),
        ])
        .expect_err("expected missing mock-quota value error");
        assert!(err.contains("missing value for --mock-quota"));
    }

    #[test]
    fn parse_args_rejects_invalid_port_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--port".to_string(),
            "abc".to_string(),
        ])
        .expect_err("expected invalid port error");
        assert!(err.contains("invalid --port"));
    }

    #[test]
    fn parse_args_rejects_missing_port_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--port".to_string(),
        ])
        .expect_err("expected missing port value error");
        assert!(err.contains("missing value for --port"));
    }

    #[test]
    fn parse_args_rejects_invalid_probe_timeout_secs_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--probe-timeout-secs".to_string(),
            "abc".to_string(),
        ])
        .expect_err("expected invalid probe timeout error");
        assert!(err.contains("invalid --probe-timeout-secs"));
    }

    #[test]
    fn parse_args_rejects_missing_probe_timeout_secs_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--probe-timeout-secs".to_string(),
        ])
        .expect_err("expected missing probe timeout value error");
        assert!(err.contains("missing value for --probe-timeout-secs"));
    }

    #[test]
    fn parse_args_rejects_missing_agent_name_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--agent-name".to_string(),
        ])
        .expect_err("expected missing agent-name value error");
        assert!(err.contains("missing value for --agent-name"));
    }

    #[test]
    fn parse_args_rejects_missing_env_file_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--env-file".to_string(),
        ])
        .expect_err("expected missing env-file value error");
        assert!(err.contains("missing value for --env-file"));
    }

    #[test]
    fn parse_args_rejects_missing_probe_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--probe".to_string(),
        ])
        .expect_err("expected missing probe value error");
        assert!(err.contains("missing value for --probe"));
    }

    #[test]
    fn parse_args_rejects_missing_env_override_value() {
        let err = parse_args(vec![
            "--model".to_string(),
            "/free/moonshotai/kimi-k2".to_string(),
            "--env".to_string(),
        ])
        .expect_err("expected missing env override value error");
        assert!(err.contains("missing value for --env"));
    }
}
