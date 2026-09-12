# Observation

Bridges watch wakeups to binding callbacks without a worker thread. Callbacks
schedule a native refresh and return false to detach. This crate owns no product
state or transitions.
