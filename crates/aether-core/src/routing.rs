//! Routing presets and user rules, compiled into Xray `routing.rules`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const TAG_PROXY: &str = "proxy";
pub const TAG_DIRECT: &str = "direct";
pub const TAG_BLOCK: &str = "block";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingPreset {
    /// Everything through the proxy (LAN optionally bypassed).
    Global,
    /// Region-local sites direct, the rest through the proxy.
    #[default]
    Smart,
    /// Everything direct except what user rules send to the proxy.
    Direct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    Domain,
    Ip,
    Process,
    Port,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleTarget {
    Proxy,
    Direct,
    Block,
}

impl RuleTarget {
    pub fn tag(self) -> &'static str {
        match self {
            RuleTarget::Proxy => TAG_PROXY,
            RuleTarget::Direct => TAG_DIRECT,
            RuleTarget::Block => TAG_BLOCK,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub kind: RuleKind,
    pub values: Vec<String>,
    pub target: RuleTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RoutingSettings {
    pub preset: RoutingPreset,
    /// Two-letter region whose sites go direct in the `Smart` preset.
    pub region: String,
    pub bypass_lan: bool,
    pub block_ads: bool,
    /// Xray `domainStrategy`: `AsIs`, `IPIfNonMatch` or `IPOnDemand`.
    pub domain_strategy: String,
    pub rules: Vec<Rule>,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        Self {
            preset: RoutingPreset::Smart,
            region: "ru".into(),
            bypass_lan: true,
            block_ads: true,
            domain_strategy: "IPIfNonMatch".into(),
            rules: Vec::new(),
        }
    }
}

/// geosite/geoip lists that represent "local" traffic for a region.
pub fn region_lists(region: &str) -> (Vec<String>, Vec<String>) {
    let r = region.to_ascii_lowercase();
    let sites = match r.as_str() {
        "ru" => vec!["geosite:category-ru".to_string()],
        "cn" => vec!["geosite:cn".to_string()],
        "ir" => vec!["geosite:category-ir".to_string()],
        _ => vec![],
    };
    (sites, vec![format!("geoip:{r}")])
}

/// Normalises a user-entered domain into Xray matcher syntax.
fn domain_matcher(v: &str) -> String {
    const PREFIXES: [&str; 6] = ["domain:", "full:", "regexp:", "keyword:", "geosite:", "ext:"];
    if PREFIXES.iter().any(|p| v.starts_with(p)) {
        v.to_string()
    } else {
        format!("domain:{}", v.trim_start_matches("*."))
    }
}

fn user_rule(rule: &Rule) -> Option<Value> {
    let values: Vec<String> = rule
        .values
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(String::from)
        .collect();
    if !rule.enabled || values.is_empty() {
        return None;
    }
    let mut v = json!({ "outboundTag": rule.target.tag() });
    match rule.kind {
        RuleKind::Domain => v["domain"] = values.iter().map(|d| domain_matcher(d)).collect(),
        RuleKind::Ip => v["ip"] = values.into(),
        RuleKind::Process => v["process"] = values.into(),
        RuleKind::Port => v["port"] = values.join(",").into(),
    }
    Some(v)
}

/// Compiles the full ordered rule list. User rules always win over presets.
pub fn build_rules(settings: &RoutingSettings) -> Vec<Value> {
    let mut rules: Vec<Value> = settings.rules.iter().filter_map(user_rule).collect();

    if settings.block_ads {
        rules.push(json!({ "domain": ["geosite:category-ads-all"], "outboundTag": TAG_BLOCK }));
    }
    if settings.bypass_lan || settings.preset != RoutingPreset::Global {
        rules.push(json!({ "ip": ["geoip:private"], "outboundTag": TAG_DIRECT }));
        rules.push(json!({ "domain": ["geosite:private"], "outboundTag": TAG_DIRECT }));
    }
    match settings.preset {
        RoutingPreset::Global => {}
        RoutingPreset::Smart => {
            let (sites, ips) = region_lists(&settings.region);
            if !sites.is_empty() {
                rules.push(json!({ "domain": sites, "outboundTag": TAG_DIRECT }));
            }
            rules.push(json!({ "ip": ips, "outboundTag": TAG_DIRECT }));
        }
        RoutingPreset::Direct => {
            rules.push(json!({ "network": "tcp,udp", "outboundTag": TAG_DIRECT }));
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: RuleKind, values: &[&str], target: RuleTarget) -> Rule {
        Rule {
            id: "r".into(),
            enabled: true,
            kind,
            values: values.iter().map(|s| s.to_string()).collect(),
            target,
            note: None,
        }
    }

    #[test]
    fn user_rules_come_first_and_are_normalised() {
        let settings = RoutingSettings {
            rules: vec![
                rule(RuleKind::Domain, &["*.openai.com", "geosite:netflix", " "], RuleTarget::Proxy),
                rule(RuleKind::Port, &["22", "3389"], RuleTarget::Direct),
                Rule { enabled: false, ..rule(RuleKind::Ip, &["1.1.1.1"], RuleTarget::Block) },
            ],
            ..Default::default()
        };
        let rules = build_rules(&settings);
        assert_eq!(rules[0]["domain"], json!(["domain:openai.com", "geosite:netflix"]));
        assert_eq!(rules[0]["outboundTag"], "proxy");
        assert_eq!(rules[1]["port"], "22,3389");
        assert!(rules.iter().all(|r| r["ip"] != json!(["1.1.1.1"])), "disabled rule leaked");
    }

    #[test]
    fn presets() {
        let global = RoutingSettings {
            preset: RoutingPreset::Global,
            bypass_lan: false,
            block_ads: false,
            ..Default::default()
        };
        assert!(build_rules(&global).is_empty());

        let smart = build_rules(&RoutingSettings::default());
        assert!(smart.contains(&json!({ "domain": ["geosite:category-ru"], "outboundTag": "direct" })));
        assert!(smart.contains(&json!({ "ip": ["geoip:ru"], "outboundTag": "direct" })));

        let direct = build_rules(&RoutingSettings { preset: RoutingPreset::Direct, ..Default::default() });
        assert_eq!(direct.last().unwrap()["outboundTag"], "direct");
    }
}
