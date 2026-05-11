//! Router: turns a [`RoutingContext`] into a [`RoutingDecision`] given the
//! current [`ConfigDocument`]. Pure function over data — no IO.

use serde::{Deserialize, Serialize};

use crate::browser::BrowserTarget;
use crate::config::ConfigDocument;
use crate::rules::{Action, MatcherTree, Rule, RuleId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingContext {
    pub url: String,
    pub source: Source,
    #[serde(default)]
    pub navigation: Option<Navigation>,
    #[serde(default)]
    pub environment: Option<Environment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    #[serde(rename = "type")]
    pub kind: SourceKind,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub browser: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    System,
    BrowserExtension,
    Cli,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Navigation {
    pub transition_type: Option<String>,
    pub is_new_tab: Option<bool>,
    pub is_redirect: Option<bool>,
    pub opener_url: Option<String>,
    pub referrer_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Environment {
    pub network: Option<String>,
    pub vpn: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum RoutingDecision {
    Open {
        target: BrowserTarget,
        matched_rule: Option<RuleId>,
        reason: String,
    },
    Allow {
        reason: String,
    },
    Ask {
        candidates: Vec<BrowserTarget>,
        reason: String,
    },
    Block {
        reason: String,
    },
}

/// Stateless evaluator over a [`ConfigDocument`].
pub struct Router<'a> {
    config: &'a ConfigDocument,
}

impl<'a> Router<'a> {
    pub fn new(config: &'a ConfigDocument) -> Self {
        Self { config }
    }

    pub fn evaluate(&self, ctx: &RoutingContext) -> RoutingDecision {
        let mut candidates: Vec<&Rule> = self
            .config
            .rules
            .iter()
            .filter(|r| r.enabled && match_tree(&r.when, ctx))
            .collect();

        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        if let Some(rule) = candidates.first() {
            return decide_from_action(&rule.then, Some(rule.id.clone()), &rule.note);
        }

        RoutingDecision::Open {
            target: self.config.default_target.clone(),
            matched_rule: None,
            reason: "default target (no rule matched)".to_string(),
        }
    }
}

fn decide_from_action(
    action: &Action,
    matched_rule: Option<RuleId>,
    note: &Option<String>,
) -> RoutingDecision {
    let reason = note.clone().unwrap_or_else(|| "matched rule".to_string());
    match action {
        Action::Open { target } => RoutingDecision::Open {
            target: target.clone(),
            matched_rule,
            reason,
        },
        Action::KeepSource => RoutingDecision::Allow { reason },
        Action::Ask => RoutingDecision::Ask {
            candidates: Vec::new(),
            reason,
        },
        Action::Block => RoutingDecision::Block { reason },
    }
}

fn match_tree(tree: &MatcherTree, ctx: &RoutingContext) -> bool {
    match tree {
        MatcherTree::Always => true,
        MatcherTree::All { of } => of.iter().all(|m| match_tree(m, ctx)),
        MatcherTree::Any { of } => of.iter().any(|m| match_tree(m, ctx)),
        MatcherTree::Not { of } => !match_tree(of, ctx),
        MatcherTree::UrlHost { pattern } => match_host(pattern, &ctx.url),
        MatcherTree::UrlPath { pattern } => match_path(pattern, &ctx.url),
        MatcherTree::SourceApp { name } => ctx
            .source
            .app_name
            .as_deref()
            .map(|n| n.eq_ignore_ascii_case(name))
            .unwrap_or(false),
        MatcherTree::SourceBrowser { browser } => ctx
            .source
            .browser
            .as_deref()
            .map(|b| b.eq_ignore_ascii_case(browser))
            .unwrap_or(false),
        MatcherTree::SourceProfile { profile } => ctx
            .source
            .profile
            .as_deref()
            .map(|p| p == profile)
            .unwrap_or(false),
    }
}

fn match_host(pattern: &str, url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    glob_match(pattern, host)
}

fn match_path(pattern: &str, url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    glob_match(pattern, parsed.path())
}

/// Minimal glob: supports a single leading `*.` wildcard for hosts and `*`
/// suffix/prefix elsewhere. Sufficient for v0.1; richer matching arrives in v0.2.
fn glob_match(pattern: &str, value: &str) -> bool {
    if let Some(rest) = pattern.strip_prefix("*.") {
        return value == rest || value.ends_with(&format!(".{rest}"));
    }
    if let Some(rest) = pattern.strip_suffix('*') {
        return value.starts_with(rest);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserId;
    use crate::rules::{Action, MatcherTree, Rule, RuleSource};

    fn rule(host: &str, target_browser: &str) -> Rule {
        Rule {
            id: RuleId::default(),
            priority: 0,
            enabled: true,
            when: MatcherTree::UrlHost {
                pattern: host.to_string(),
            },
            then: Action::Open {
                target: BrowserTarget::new(BrowserId::new(target_browser)),
            },
            source: RuleSource::Gui,
            note: None,
        }
    }

    fn ctx(url: &str) -> RoutingContext {
        RoutingContext {
            url: url.to_string(),
            source: Source {
                kind: SourceKind::System,
                app_name: None,
                bundle_id: None,
                browser: None,
                profile: None,
            },
            navigation: None,
            environment: None,
        }
    }

    #[test]
    fn falls_back_to_default_target() {
        let config = ConfigDocument::with_default(BrowserTarget::new(BrowserId::new("arc")));
        let router = Router::new(&config);
        let decision = router.evaluate(&ctx("https://example.com/"));
        match decision {
            RoutingDecision::Open { target, .. } => assert_eq!(target.browser.0, "arc"),
            other => panic!("expected Open, got {other:?}"),
        }
    }

    #[test]
    fn matches_host_rule() {
        let mut config = ConfigDocument::with_default(BrowserTarget::new(BrowserId::new("arc")));
        config.rules.push(rule("github.com", "chrome"));
        let router = Router::new(&config);
        let decision = router.evaluate(&ctx("https://github.com/x/y"));
        match decision {
            RoutingDecision::Open { target, .. } => assert_eq!(target.browser.0, "chrome"),
            other => panic!("expected Open, got {other:?}"),
        }
    }

    #[test]
    fn glob_matches_subdomain() {
        assert!(glob_match("*.corp.example.com", "wiki.corp.example.com"));
        assert!(!glob_match("*.corp.example.com", "example.com"));
    }
}
