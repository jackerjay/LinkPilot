//! Route history / inspector data model. v0.1 step 1 only defines the shape;
//! the in-memory ring buffer and persistence come with the Inspector page.

use serde::{Deserialize, Serialize};

use crate::routing::{RoutingContext, RoutingDecision};
use crate::rules::RuleId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRecord {
    pub timestamp_ms: u64,
    pub context: RoutingContext,
    pub decision: RoutingDecision,
    pub matched_rule: Option<RuleId>,
}
