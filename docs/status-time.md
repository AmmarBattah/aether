# Current-state time

`Agent.status_since` is the start of the **current continuous status kind**,
not agent lifetime, model execution time, or the age of a particular wait
condition. New runtime-created agents initialize it from their creation clock.
A successful kind transition replaces it with that transition's timestamp;
same-kind reclassification preserves it. Failed transitions change nothing.
The existing transition graph is unchanged: Waiting -> Waiting is forbidden;
`merge_wait` combines pending turn effects before a single Waiting transition.
Editing the condition without leaving Waiting does not start a new period.
Waiting -> Ready -> Running -> Waiting does, even if the final condition matches.

`updated_at` is deliberately unrelated: usage, inbox, checkpoint, pause, name,
and budget updates must not restart this clock. Turn row merges copy status and
status_since together, preserving concurrent terminal decisions and operator
configuration. Resource-block entry/exit holds the write gate across its read
and commit, and copies the pair back to the turn only after persistence succeeds.
Recovery uses normal transitions. Imported history whose kind is changed to
Imported has unknown status-entry time, rather than inheriting the old kind's age.

## Compatibility and data flow

The field is optional, defaults to None, and is omitted when unknown. Old JSON
rows load without migration. No fallback to created_at or updated_at is valid.
The optional field is exposed through the generated JSON schema, Python, and
TypeScript clients; protocol version 11 / minimum 10 remain unchanged.

Agent rows are stored as JSON. GetTree's nodes contain the complete stored Agent.
The TUI event reducer does not locally assign agent statuses: relevant events
request a coalesced tree refresh, and apply_tree replaces the complete snapshot.
Consequently status and status_since arrive together on late attach, ordinary
refresh, and reconnect, including a same-kind ABA change missed between frames.
No per-row request or event-observation timestamp is introduced.

## Presentation

The TUI board's elapsed column shows how long a live agent has been in its
current status, and a finished agent's total runtime (creation to its final
status). Legacy state without `status_since` leaves the column blank rather
than guessing. Durations use the frame's `app.now`, with future clock skew
clamped to zero. They are wall-clock status residence, not CPU/provider time;
pauses are orthogonal.

No new polling, wakeup, or animation timer is added. Ages refresh on existing
rendered frames (input, subscription replies/events, and existing
housekeeping/motion frames); an otherwise unchanged idle screen is not
guaranteed second-by-second updates. Seconds display below a minute, then
minutes and seconds (`2m 40s`), then hours and minutes (`1h 01m`).
