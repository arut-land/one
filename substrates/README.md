# Substrates

Substrates are small mechanisms reused by otherwise independent features. They do not contain product meaning or select policy.

`watch` owns revisioned snapshots and coalesced invalidation subscriptions over Tokio watch. Feature crates define the concrete state records and transitions that use it.

Add a substrate only after at least one concrete feature needs the mechanism and its ownership is clearly outside that feature.
