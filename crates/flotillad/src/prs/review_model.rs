//! A review as the daemon keeps it (H-261 §1.3): a role's verdict on one
//! head, bound to that head's patch-id. It goes stale when the PR's own
//! change differs (§3); it is never edited, only superseded.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::model::Pr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Approved,
    ChangesRequested,
}

impl Verdict {
    pub const ALL: [Verdict; 2] = [Self::Approved, Self::ChangesRequested];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

/// One finding of a review: `must` blocks the merge until resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// `must`, `should` or `nit`.
    pub severity: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_in: Option<String>,
    /// The Inbox card it became (`pr_follow_up`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_up_item_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub id: String,
    pub pr_id: String,
    /// `architect`, `ux`, `ce`, `devops`, `qa` or `owner`.
    pub role: String,
    /// A bot id, or `owner`.
    pub reviewer: String,
    /// The owner's `device:<id>` or `ticket`; `None` for a bot.
    pub provenance: Option<String>,
    pub sha: String,
    pub patch_id: String,
    pub verdict: Verdict,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub artifact: Option<String>,
    pub at: DateTime<Utc>,
}

impl Review {
    /// The PR's own change differs from the one reviewed (§3). Computed,
    /// never stored: an update with main that keeps the change keeps it.
    pub fn stale(&self, pr: &Pr) -> bool {
        self.patch_id != pr.head_patch_id
    }

    pub fn to_json(&self, pr: &Pr) -> Value {
        json!({
            "id": self.id, "role": self.role, "reviewer": self.reviewer,
            "provenance": self.provenance, "sha": self.sha,
            "verdict": self.verdict.as_str(), "summary": self.summary,
            "findings": self.findings, "artifact": self.artifact,
            "stale": self.stale(pr), "at": self.at,
        })
    }
}

/// Marks each `must` or `should` finding resolved in the commit the same
/// role approved after it (0.24.0): an approval carries no such finding
/// open, so approving a later head means it was fixed there. `reviews` are
/// in the order they were given.
pub fn resolve_findings(reviews: &mut [Review]) {
    for later in 0..reviews.len() {
        if reviews[later].verdict != Verdict::Approved {
            continue;
        }
        let (role, sha) = (reviews[later].role.clone(), reviews[later].sha.clone());
        for earlier in reviews[..later].iter_mut().filter(|r| r.role == role) {
            for finding in &mut earlier.findings {
                if finding.severity != "nit" && finding.resolved_in.is_none() {
                    finding.resolved_in = Some(sha.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(role: &str, sha: &str, verdict: Verdict, severities: &[&str]) -> Review {
        Review {
            id: format!("{role}-{sha}"),
            pr_id: "pr".into(),
            role: role.into(),
            reviewer: "bot".into(),
            provenance: None,
            sha: sha.into(),
            patch_id: sha.into(),
            verdict,
            summary: String::new(),
            findings: severities
                .iter()
                .map(|s| Finding {
                    severity: (*s).into(),
                    text: format!("a {s}"),
                    path: None,
                    line: None,
                    resolved_in: None,
                    follow_up_item_id: None,
                })
                .collect(),
            artifact: None,
            at: Utc::now(),
        }
    }

    #[test]
    fn a_later_approval_by_the_same_role_resolves_its_findings() {
        let mut reviews = vec![
            review(
                "architect",
                "a1",
                Verdict::ChangesRequested,
                &["should", "nit"],
            ),
            review("ux", "a1", Verdict::ChangesRequested, &["must"]),
            review("architect", "b2", Verdict::Approved, &[]),
        ];
        resolve_findings(&mut reviews);
        let architect = &reviews[0].findings;
        assert_eq!(architect[0].resolved_in.as_deref(), Some("b2"));
        assert_eq!(architect[1].resolved_in, None, "a nit stays a nit");
        assert_eq!(
            reviews[1].findings[0].resolved_in, None,
            "UX hasn't approved since"
        );
    }
}
