def can_spend(lead, cost):
    return (lead.spent_cents + cost) <= lead.budget_cents