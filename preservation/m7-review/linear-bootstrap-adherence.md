# M5 direct-layer bootstrap adherence

Scope c202434; production bootstrap/formatter/storage source unchanged from
9a70e50. M4 prerequisite is closed. These are three focused self-reviews using
existing evidence, not independent certification or repeated test executions.

1. Forward construction: checked DESIGN bootstrap rules against
   BootstrapBuild::produce, Ext4Format and BootstrapVolumes. The supervisor
   formats a snapshot over dm-zero with typed ext4 capacity/label and captured
   trusted runtime/configuration. Tool and output snapshots are separate; the
   same pair owner persists across directives with fresh mounts per command.
   Only output is sealed with zero parent root. Tool writes and components are
   discarded, no image-to-scute conversion runs. Immutable profile values,
   sealed Volume states, private fields and consuming transitions satisfy style.
   The current full pipeline receipt confirms tools are not inherited and high
   numeric IDs survive sealing/remount. No new defect. Clean pass 1.

2. Reverse failure: checked bootstrap rejection, formatter wait/cancel, paired
   mount rollback/unmount and finalization. Formatter owners remain retained
   until reap; failure keeps its initial cleanup cutoff. Partial pair failures
   cannot yield independently extractable accepted output. Terminal discard
   attempts both roots, retains diagnostics and cannot publish. M4's command
   success/cleanup separation remains the prerequisite. Existing formatter
   failure evidence remains relevant: profile and paired storage are unchanged
   from the previous reviewed implementation; execution only lost an unused
   accessor. Current bootstrap nonzero and second-root label-denial evidence
   confirms rejection, no later admission and absent output. No new defect or
   Rust ownership/style regression. Clean pass 2.

3. Requirements/evidence challenge: compared typed size bounds, fixed logical
   geometry, label-as-argument handling, captured formatter receipt and direct
   sealing with DESIGN and the maintained pipeline fixture. Current
   environment-id-maps/verification.log establishes output/tool separation,
   emitted inventory/blob graph and one carapace-stage scute. Current
   linear-lifecycle/labels and label-second-root cover both assigned roots under
   the environment policy. Historical metadata/formatter tests are supporting
   unchanged-code evidence, not claims of new execution or distro qualification.
   Source remains safe Rust and owning methods/typestate; no references or
   validation inputs were retired. All relevant changes have source/evidence
   coverage; no new test is warranted. Clean pass 3.

M5 closes at 3/3. M6 pipeline/publication, M7 distro/platform qualification and
M8 durable preservation/retirement remain separate outstanding work.
