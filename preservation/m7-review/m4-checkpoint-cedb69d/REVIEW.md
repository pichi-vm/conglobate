# M4 requirement/evidence checkpoint

Status: not a complete adherence pass. UID/GID supported-range decision remains
pending, and the full original-finding reconciliation/platform requirements are
not finished. No milestone advancement from passing tests.

Current implementation-level cleanup regression was resolved at cedb69d. This
checkpoint runs the complete rootless all-target suite and doctests after that
protocol change; exact totals and logs are retained. It does not rerun unrelated
privileged tests or relabel previous VM evidence as current execution.

Source inspection confirms RuntimeFilesystem::EmptySys creates an empty readonly
nosuid/nodev/noexec tmpfs and attaches it through pinned handles; it is not a bind
of supervisor sysfs/debugfs. CommandRoot invokes this before pivot-root/admission.
The official kernel scheduler documentation places CONFIG_SCHED_DEBUG interfaces
under /sys/kernel/debug/sched:
https://www.kernel.org/doc/html/v6.11/scheduler/sched-debug.html
(accessed 2026-10-23). The kernel.org historical commit URL for the proc-to-debugfs
move was inaccessible through the research tool, so it is not used as proof of a
specific version boundary. No new minimum supported kernel is inferred.

The installed 7.2.10 config has DEBUG_FS entries but no SCHED_DEBUG matching line;
this observation does not establish behavior for alternate configurations or old
kernels with /proc/sched_debug. Existing tested proc absence remains qualified
only for that tested environment. security-003 stays partially qualified; do not
invent a mandatory kernel downgrade or declare that missing coverage passed.

README runtime wording still referred to scoped policy hooks. Corrected it to
match the environment-loaded labeling exception already adopted in DESIGN and
SANDBOX-DESIGN. No binary policy installation is introduced.

The accepted dependencies-002 action was to document the actual advisory-source
matching limitation, not certify all Git dependencies. README already states that
clean audit results do not certify pinned Git revisions against ordinary registry
advisories. Close that documentation action on source evidence. Fresh advisories,
manual revision review and final dependency qualification remain separate M7 work;
no fresh cargo-audit success is claimed here.
