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

/// Pair of (decision, explanation). [`Router::evaluate_explained`] returns
/// this so [`history::RouteRecord`] can attach a per-node "did this match?"
/// trace to every routed URL — driving the Inspector "explain why" UI.
/// Serializes for the Tauri `route_evaluate` command so the GUI Test-URL
/// panel can show the same trace without launching a browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explained {
    pub decision: RoutingDecision,
    /// `Some` when a user-authored rule won. `None` when the default target
    /// fired (no rules matched) — there is no matcher tree to annotate.
    pub explanation: Option<MatcherEval>,
}

/// Stateless evaluator over a [`ConfigDocument`].
pub struct Router<'a> {
    config: &'a ConfigDocument,
}

impl<'a> Router<'a> {
    pub fn new(config: &'a ConfigDocument) -> Self {
        Self { config }
    }

    /// Convenience: just the decision. Keeps existing IPC / CLI callsites
    /// unchanged when they don't need an explanation.
    pub fn evaluate(&self, ctx: &RoutingContext) -> RoutingDecision {
        self.evaluate_explained(ctx).decision
    }

    pub fn evaluate_explained(&self, ctx: &RoutingContext) -> Explained {
        // Master kill-switch: when smart routing is off we don't even
        // peek at the rules — open in default_target and report it
        // honestly so the Inspector / history can show *why* a route
        // bypassed an obviously-matching rule.
        if !self.config.settings.smart_routing_enabled {
            return Explained {
                decision: RoutingDecision::Open {
                    target: self.config.default_target.clone(),
                    matched_rule: None,
                    reason: "smart routing disabled — default target".to_string(),
                },
                explanation: None,
            };
        }
        // Walk rules in declared order, collecting (priority, rule, eval)
        // for those that match. We have to eval every enabled rule's tree
        // anyway to know whether it matched, so we get the explanation for
        // free.
        let mut hits: Vec<(&Rule, MatcherEval)> = Vec::new();
        for rule in &self.config.rules {
            if !rule.enabled {
                continue;
            }
            // Workspace gate: a rule in a disabled workspace is silently
            // skipped (same as `enabled = false`). Missing-workspace
            // dangling refs default to enabled so an accidentally
            // orphaned rule keeps working — the delete path clears the
            // field anyway.
            if let Some(ws_id) = &rule.workspace_id {
                let ws_enabled = self
                    .config
                    .workspaces
                    .iter()
                    .find(|w| &w.id == ws_id)
                    .map(|w| w.enabled)
                    .unwrap_or(true);
                if !ws_enabled {
                    continue;
                }
            }
            let eval = eval_tree(&rule.when, ctx);
            if eval.matched() {
                hits.push((rule, eval));
            }
        }
        hits.sort_by(|a, b| b.0.priority.cmp(&a.0.priority));

        if let Some((rule, eval)) = hits.into_iter().next() {
            return Explained {
                decision: decide_from_action(&rule.then, Some(rule.id.clone()), &rule.note),
                explanation: Some(eval),
            };
        }

        Explained {
            decision: RoutingDecision::Open {
                target: self.config.default_target.clone(),
                matched_rule: None,
                reason: "default target (no rule matched)".to_string(),
            },
            explanation: None,
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

/// Per-node match trace. Same shape as [`MatcherTree`], plus a `matched`
/// flag at every node — the frontend renders this as a highlighted
/// boolean tree in the Inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum MatcherEval {
    Always { matched: bool },
    All { matched: bool, of: Vec<MatcherEval> },
    Any { matched: bool, of: Vec<MatcherEval> },
    Not { matched: bool, of: Box<MatcherEval> },
    UrlHost { matched: bool, pattern: String },
    UrlPath { matched: bool, pattern: String },
    SourceApp { matched: bool, name: String },
    SourceBrowser { matched: bool, browser: String },
    SourceProfile { matched: bool, profile: String },
}

impl MatcherEval {
    pub fn matched(&self) -> bool {
        match self {
            Self::Always { matched }
            | Self::All { matched, .. }
            | Self::Any { matched, .. }
            | Self::Not { matched, .. }
            | Self::UrlHost { matched, .. }
            | Self::UrlPath { matched, .. }
            | Self::SourceApp { matched, .. }
            | Self::SourceBrowser { matched, .. }
            | Self::SourceProfile { matched, .. } => *matched,
        }
    }
}

fn eval_tree(tree: &MatcherTree, ctx: &RoutingContext) -> MatcherEval {
    match tree {
        MatcherTree::Always => MatcherEval::Always { matched: true },
        MatcherTree::All { of } => {
            let children: Vec<MatcherEval> = of.iter().map(|m| eval_tree(m, ctx)).collect();
            let matched = !children.is_empty() && children.iter().all(|c| c.matched());
            MatcherEval::All {
                matched,
                of: children,
            }
        }
        MatcherTree::Any { of } => {
            let children: Vec<MatcherEval> = of.iter().map(|m| eval_tree(m, ctx)).collect();
            let matched = children.iter().any(|c| c.matched());
            MatcherEval::Any {
                matched,
                of: children,
            }
        }
        MatcherTree::Not { of } => {
            let child = eval_tree(of, ctx);
            let matched = !child.matched();
            MatcherEval::Not {
                matched,
                of: Box::new(child),
            }
        }
        MatcherTree::UrlHost { pattern } => MatcherEval::UrlHost {
            matched: match_host(pattern, &ctx.url),
            pattern: pattern.clone(),
        },
        MatcherTree::UrlPath { pattern } => MatcherEval::UrlPath {
            matched: match_path(pattern, &ctx.url),
            pattern: pattern.clone(),
        },
        MatcherTree::SourceApp { name, bundle_id } => {
            // Bundle id is the stable identifier (e.g. com.electron.lark
            // matches whether the localized name is "Lark", "Feishu", or
            // "飞书"). Only fall back to name matching when no bundle id
            // is stored on the rule — old configs authored by hand, or
            // rules where the user typed a name without using the picker.
            let matched = match bundle_id {
                Some(bid) if !bid.is_empty() => ctx
                    .source
                    .bundle_id
                    .as_deref()
                    .map(|b| b.eq_ignore_ascii_case(bid))
                    .unwrap_or(false),
                _ => ctx
                    .source
                    .app_name
                    .as_deref()
                    .map(|n| n.eq_ignore_ascii_case(name))
                    .unwrap_or(false),
            };
            MatcherEval::SourceApp {
                matched,
                name: name.clone(),
            }
        }
        MatcherTree::SourceBrowser { browser } => MatcherEval::SourceBrowser {
            matched: ctx
                .source
                .browser
                .as_deref()
                .map(|b| b.eq_ignore_ascii_case(browser))
                .unwrap_or(false),
            browser: browser.clone(),
        },
        MatcherTree::SourceProfile { profile } => MatcherEval::SourceProfile {
            matched: ctx
                .source
                .profile
                .as_deref()
                .map(|p| p == profile)
                .unwrap_or(false),
            profile: profile.clone(),
        },
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
            workspace_id: None,
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

    #[test]
    fn explanation_annotates_each_node() {
        // AND(host=github.com, source-app=Slack)
        // Context: github URL from Slack → both leaves match, root matches.
        let mut config = ConfigDocument::with_default(BrowserTarget::new(BrowserId::new("arc")));
        config.rules.push(Rule {
            id: RuleId::default(),
            priority: 10,
            enabled: true,
            when: MatcherTree::All {
                of: vec![
                    MatcherTree::UrlHost {
                        pattern: "github.com".into(),
                    },
                    MatcherTree::SourceApp {
                        name: "Slack".into(),
                        bundle_id: None,
                    },
                ],
            },
            then: Action::Open {
                target: BrowserTarget::new(BrowserId::new("chrome")),
            },
            source: RuleSource::Gui,
            note: None,
            workspace_id: None,
        });
        let mut c = ctx("https://github.com/x/y");
        c.source.app_name = Some("Slack".into());
        let explained = Router::new(&config).evaluate_explained(&c);

        let eval = explained.explanation.expect("rule should fire");
        assert!(eval.matched());
        match eval {
            MatcherEval::All { matched, of } => {
                assert!(matched);
                assert_eq!(of.len(), 2);
                assert!(of[0].matched());
                assert!(of[1].matched());
            }
            other => panic!("expected All, got {other:?}"),
        }
    }

    #[test]
    fn disabled_workspace_skips_its_rules() {
        use crate::config::Workspace;
        let mut config = ConfigDocument::with_default(BrowserTarget::new(BrowserId::new("arc")));
        config.workspaces.push(Workspace {
            id: "work".into(),
            display_name: "Work".into(),
            description: None,
            enabled: false,
        });
        let mut r = rule("github.com", "chrome");
        r.workspace_id = Some("work".into());
        config.rules.push(r);
        let decision = Router::new(&config).evaluate(&ctx("https://github.com/x"));
        match decision {
            RoutingDecision::Open {
                target,
                matched_rule,
                ..
            } => {
                // Workspace off → rule skipped → default target (arc) wins.
                assert_eq!(target.browser.0, "arc");
                assert!(matched_rule.is_none());
            }
            other => panic!("expected default open, got {other:?}"),
        }
    }

    #[test]
    fn enabled_workspace_lets_rule_through() {
        use crate::config::Workspace;
        let mut config = ConfigDocument::with_default(BrowserTarget::new(BrowserId::new("arc")));
        config.workspaces.push(Workspace {
            id: "work".into(),
            display_name: "Work".into(),
            description: None,
            enabled: true,
        });
        let mut r = rule("github.com", "chrome");
        r.workspace_id = Some("work".into());
        config.rules.push(r);
        let decision = Router::new(&config).evaluate(&ctx("https://github.com/x"));
        match decision {
            RoutingDecision::Open { target, .. } => assert_eq!(target.browser.0, "chrome"),
            other => panic!("expected chrome, got {other:?}"),
        }
    }

    #[test]
    fn explanation_marks_failing_child_in_and() {
        // AND(host=github.com, source-app=Slack)
        // Context: github URL but NO source app → host matches, source fails,
        // root fails → no rule fires, explanation is None.
        let mut config = ConfigDocument::with_default(BrowserTarget::new(BrowserId::new("arc")));
        config.rules.push(Rule {
            id: RuleId::default(),
            priority: 10,
            enabled: true,
            when: MatcherTree::All {
                of: vec![
                    MatcherTree::UrlHost {
                        pattern: "github.com".into(),
                    },
                    MatcherTree::SourceApp {
                        name: "Slack".into(),
                        bundle_id: None,
                    },
                ],
            },
            then: Action::Open {
                target: BrowserTarget::new(BrowserId::new("chrome")),
            },
            source: RuleSource::Gui,
            note: None,
            workspace_id: None,
        });
        let explained = Router::new(&config).evaluate_explained(&ctx("https://github.com/x/y"));
        assert!(explained.explanation.is_none(), "no rule should have fired");
        // And the decision should be the default target (arc), not chrome.
        match explained.decision {
            RoutingDecision::Open {
                target,
                matched_rule,
                ..
            } => {
                assert_eq!(target.browser.0, "arc");
                assert!(matched_rule.is_none());
            }
            other => panic!("expected Open default, got {other:?}"),
        }
    }
}
