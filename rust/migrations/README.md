
# Expanations

> Each table should have some responsibilities

## `billings`

**billings** tables should never decide prices.
They only record what already happened.

## `deployment_presets`

**deployment_presets** define defaults, minimums, and expectations — not pricing truth.

They are UX constructs, not billing constructs.

Therefore, presets SHOULD:

* Define:
  * base cpu
  * base memory
  * optional bias (discount or uplift)
* Enforce:
  * minimum CPU/RAM
  * sane combinations
* Anchor:
  * "Growth feels like this"
  * "Business feels like that"

Presets SHOULD NOT:

* Be the authoritative source of price
* Compete with add-ons numerically
* Encode currency logic

## The "Confusing" Tables: Who Does What?

Think of it like a restaurant:

    deployments (The Order): "I want a Steak (4 vCPU) and a Coke (1GB RAM)."

    billings (The Receipt): "You sat here for 2 hours. Steak cost 50k, Coke cost 10k. Total owed: 60k."
    
    transactions (The Wallet): "You paid 60k via Click. Your balance is now 0."

Table Responsibility Why is it separate?
deployments State of the World. Stores the Desired State (what the user wants). It doesn't care about money. It only cares about Kubernetes.
billings The Calculator. It runs every hour. It looks at deployments, calculates (price * hours), and saves a "Snapshot". Critical: If you change the price of the "Starter" plan tomorrow, you must NOT change the history of what users paid yesterday. billings locks in the cost at that moment.
transactions The Ledger. It is the only table allowed to touch the balances table. It mixes "Usage Charges" (negative money) from the billings table with "Top-ups" (positive money) from Payme.
