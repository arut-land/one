# HTTP transport

The HTTP channel runs on the caller's executor. It creates no runtime. Native
composition roots can wrap it in the local runtime's ScheduledChannel for callers
that poll futures outside Tokio. Connect framing replaces the legacy wire format
in step 5.
