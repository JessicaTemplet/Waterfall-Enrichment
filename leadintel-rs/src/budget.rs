//! Budget guard — checks whether a lead can afford a stage's cost.
//!
//! Python equivalent: `leadintel/core/budget.py`
//!
//!     def can_spend(lead, cost):
//!         return (lead.spent_cents + cost) <= lead.budget_cents
//!
//! This is intentionally tiny — one pure function, easy to test.
//! In Python it's a one-liner; in Rust it's equally simple.

use crate::models::Lead;

/// Return true if spending `cost_cents` would not exceed the lead's budget.
///
/// Python equivalent:
///     def can_spend(lead, cost):
///         return (lead.spent_cents + cost) <= lead.budget_cents
///
/// We take a reference `&Lead` (read-only borrow) rather than taking ownership,
/// because we just need to read the numbers — we don't want to move the Lead
/// value into this function and have the caller lose it.
pub fn can_spend(lead: &Lead, cost_cents: i64) -> bool {
    lead.spent_cents + cost_cents <= lead.budget_cents
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lead_with_budget(budget: i64, spent: i64) -> Lead {
        let mut l = Lead::new("Test", "Corp");
        l.budget_cents = budget;
        l.spent_cents  = spent;
        l
    }

    #[test]
    fn within_budget() {
        let lead = lead_with_budget(25, 10);
        assert!(can_spend(&lead, 10));  // 10 + 10 = 20 <= 25
    }

    #[test]
    fn exactly_at_budget() {
        let lead = lead_with_budget(25, 17);
        assert!(can_spend(&lead, 8));   // 17 + 8 = 25 <= 25
    }

    #[test]
    fn over_budget() {
        let lead = lead_with_budget(25, 20);
        assert!(!can_spend(&lead, 8)); // 20 + 8 = 28 > 25
    }
}
