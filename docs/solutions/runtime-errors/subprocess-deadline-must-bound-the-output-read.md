---
title: A subprocess deadline must bound the output read, not only the child's exit
date: 2026-11-23
category: runtime-errors
module: connections external commands
problem_type: runtime_error
component: infrastructure
symptoms:
  - "Opening a session hangs without limit when the password command starts a background process"
  - "The deadline fires on the direct child, yet the caller stays blocked joining the stdout reader thread"
root_cause: logic_error
resolution_type: code_fix
severity: high
tags: [subprocess, timeout, process-group, stdout-pipe, password-command, ssh, azure-cli]
---

# A subprocess deadline must bound the output read, not only the child's exit

## Problem
The password command, `ssh -G` and `az` each ran under a deadline, but a command that left a background descendant holding stdout open blocked session opening forever. Found by code review of PR #9 (open as Draft as of this writing).

## Symptoms
- A password command wrapper that starts a daemon (an agent, a keychain helper) makes the first query hang with no timeout error.

## What Didn't Work
- Each caller polled `child.try_wait()` until the deadline, killed the direct child on timeout, then joined a thread doing `read_to_end` on stdout. The deadline covered the exit of the direct child only. `read_to_end` returns at end-of-file, and end-of-file arrives only when every process holding the write end of the pipe has closed it, including a backgrounded grandchild. `child.kill()` does not reach that grandchild either.
- Three hand-written copies of this loop had already drifted before the fix: the `az` copy killed the child when `try_wait` errored, the password command copy did not.

## Solution
One helper, `run_bounded` in `src/contexts/connections/infrastructure/data_sources/bounded_process/mod.rs`, is the only way the connections context runs an external command:
- the command starts in its own process group (`process_group(0)`);
- a reader thread sends stdout chunks over a channel, and the caller waits with `recv_timeout` on the remaining deadline, so the read itself is bounded;
- the output is capped (`OutputTooLarge`);
- on timeout or overflow it sends `SIGKILL` to the whole group, then reaps the child.

Callers only build the `Command` and map `BoundedFailure` to their own error type.

## Why This Works
The deadline now governs the step that actually blocks (waiting for output), and killing the group closes every copy of the pipe's write end, so no descendant can outlive the deadline or keep a thread waiting.

## Prevention
- Any new external command (another token CLI, a secret manager) goes through `run_bounded`; never reintroduce a `try_wait` loop followed by a blocking `read_to_end` or `wait_with_output`.
- The module's tests hold the contract: `un_descendant_en_arriere_plan_qui_garde_la_sortie_ouverte_respecte_le_delai` backgrounds `sleep 30` while stdout stays open, `le_delai_depasse_tue_tout_le_groupe_de_processus` checks that a grandchild dies with the group, and `une_sortie_trop_volumineuse_est_refusee_et_arrete_la_commande` checks the cap.
- Group kill is Unix only (`#[cfg(unix)]`); on another platform a descendant can still survive the deadline.

## Related Issues
- PR #9 (connection file, password command, SSH aliases)
