//! Route history / inspector data model + in-memory ring buffer.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::routing::{MatcherEval, RoutingContext, RoutingDecision};
use crate::rules::RuleId;

pub const DEFAULT_CAPACITY: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRecord {
    pub timestamp_ms: u64,
    pub context: RoutingContext,
    pub decision: RoutingDecision,
    pub matched_rule: Option<RuleId>,
    /// Per-node match trace for the rule that won. `None` when no user rule
    /// fired (default-target fallback) or when the decision was constructed
    /// from a context that did not run through [`Router::evaluate_explained`].
    #[serde(default)]
    pub explanation: Option<MatcherEval>,
}

impl RouteRecord {
    pub fn new(context: RoutingContext, decision: RoutingDecision) -> Self {
        Self::with_explanation(context, decision, None)
    }

    pub fn with_explanation(
        context: RoutingContext,
        decision: RoutingDecision,
        explanation: Option<MatcherEval>,
    ) -> Self {
        let matched_rule = match &decision {
            RoutingDecision::Open { matched_rule, .. } => matched_rule.clone(),
            _ => None,
        };
        Self {
            timestamp_ms: now_ms(),
            context,
            decision,
            matched_rule,
            explanation,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Fixed-capacity ring buffer of recent route decisions. Newest first when
/// read via [`RouteHistory::recent`].
pub struct RouteHistory {
    capacity: usize,
    inner: Mutex<VecDeque<RouteRecord>>,
}

impl RouteHistory {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn log(&self, record: RouteRecord) {
        let mut guard = self.inner.lock().expect("history mutex poisoned");
        if guard.len() == self.capacity {
            guard.pop_front();
        }
        guard.push_back(record);
    }

    pub fn recent(&self, limit: usize) -> Vec<RouteRecord> {
        let guard = self.inner.lock().expect("history mutex poisoned");
        guard.iter().rev().take(limit).cloned().collect::<Vec<_>>()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for RouteHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::{BrowserId, BrowserTarget};
    use crate::routing::{Source, SourceKind};

    fn fixture() -> RouteRecord {
        RouteRecord::new(
            RoutingContext {
                url: "https://example.com".into(),
                source: Source {
                    kind: SourceKind::System,
                    app_name: None,
                    bundle_id: None,
                    browser: None,
                    profile: None,
                },
                navigation: None,
                environment: None,
            },
            RoutingDecision::Open {
                target: BrowserTarget::new(BrowserId::new("chrome")),
                matched_rule: None,
                reason: "test".into(),
            },
        )
    }

    #[test]
    fn caps_at_capacity() {
        let history = RouteHistory::with_capacity(3);
        for _ in 0..5 {
            history.log(fixture());
        }
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn returns_newest_first() {
        let history = RouteHistory::with_capacity(10);
        for i in 0..3 {
            let mut rec = fixture();
            rec.timestamp_ms = i;
            history.log(rec);
        }
        let recent = history.recent(10);
        assert_eq!(recent[0].timestamp_ms, 2);
        assert_eq!(recent[2].timestamp_ms, 0);
    }
}
