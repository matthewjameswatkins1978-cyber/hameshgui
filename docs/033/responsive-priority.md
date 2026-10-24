# Hamesh principle: responsive layouts preserve critical information

Derived from 033 review (Signal Ops at 820x560 displayed `DEGRADED` while
hiding every alert explaining why).

## The rule

When space runs out, a layout may shed **decoration and repetition**, never
**the information a user needs to act**.

A responsive breakpoint that removes a region must answer three questions:

1. **What is the state?** A count or summary survives — the header badge
   reads `DEGRADED · 3 ALERTS`, not just `DEGRADED`.
2. **What is the most important item?** The highest-priority content stays
   visible, in compact form, without scrolling or a second click.
3. **How does the user act?** The action attached to that content remains
   reachable.

Minimum acceptable collapse is therefore *state + top item + action*.
Anything less is data loss dressed up as responsiveness.

## Where this lives in Hamesh

The rule is a composition principle, not framework behaviour: Hamesh has no
concept of which alert matters. What the framework provides is the ability
to make the trade-off deliberately and verify it:

- `visual_report().off_viewport` names regions pushed out of reach, so a
  "responsive" rule that silently drops content is visible in telemetry;
- `probe(x, y)` answers what is actually at a point after the breakpoint;
- paint-time clipping guarantees that when a region does overflow, it is cut
  rather than sprayed across neighbours — and `clipped_content` names what
  was cut, so hidden content is never silent.

Application code decides *what* is critical. Hamesh makes the consequence
measurable.

## Applied in the 033 lab

| Width | Alerts |
|---|---|
| >= 1000px | full alert column with all three alerts and escalation |
| < 1000px | header badge carries the count; compact strip carries the top alert and escalation |

Never: alerts removed entirely.
