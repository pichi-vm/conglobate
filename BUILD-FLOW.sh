#!/usr/bin/env bash
# DESIGN SKETCH ONLY. Names below describe Rust operations, not installed tools.
# This file is deliberately non-executable and is not an implementation plan
# for running untrusted recipes in a shell process.
echo 'Design sketch only; do not execute.' >&2
exit 2

# ENVIRONMENT owns the VM/kernel, labeling policy, network namespace and DNS.
# It also clones/checks out Git and supplies a stable local build context.
# It destroys the VM when conglobate exits unexpectedly or cannot clean up.
# CONGLOBATE owns one build session: workspace, volumes, commands and output.
# COMMANDS own nothing outside their assigned filesystem views and processes.
# No guardian, persisted recovery journal, or recovery of a previous build.
# Every operation below either succeeds or transfers control to abort_build;
# that implicit error propagation is omitted to keep the main sequence visible.

# Failure path, conceptually wrapped around the entire flow below.
# Rust must preserve the original error as well as any cleanup error.
abort_build() {
    stop_and_reap_commands_within_teardown_deadline
    # Only after quiescence may we unmount or release their backing storage.
    # If quiescence or cleanup cannot be established: STOP, reject publication,
    # retain resources until the environment destroys the VM. Do not continue
    # tearing down dependencies underneath a potentially live command.
    unmount_and_release_owned_resources
    discard_unpublished_output
    exit 1
}
# This is explanatory, not a claim that a shell ERR/EXIT trap can implement
# those guarantees. Rust owns error propagation and partial-acquisition cleanup.

run_steps() {
    while recipe_has_next_step; do
        read_next_step
        case "$step_kind" in
            copy)
                mount_stage_volumes
                copy_from_private_context_into_assigned_root
                # Rust: confined descriptor-relative traversal; preserve required
                # metadata. Never expose the context itself to commands.
                flush_and_unmount_stage_volumes
                ;;
            run)
                mount_stage_volumes
                prepare_command_sandbox
                # Trusted setup, bounded by an internal setup deadline:
                #   create cgroup and private mount/PID/IPC/UTS/user namespaces;
                #   map image ownership and install only assigned mount views;
                #   supply minimal /dev, restricted /proc and empty /sys;
                #   join the supplied network namespace if network: true,
                #     otherwise create a disconnected network namespace;
                #   install captured resolv.conf read-only when enabled;
                #   pivot root, detach old root, close unrelated descriptors;
                #   restrict capabilities/syscalls before allowing image exec.
                # The supervisor keeps cgroup/process control descriptors.
                # Commands get no context, cache, backing devices or controls.

                start_command /bin/sh -eu -c "$step_script"
                wait_for_command_with_recipe_timeout
                # Default 5m; every command has a finite execution timeout.
                # Nonzero exit, timeout or helper failure rejects this build.

                stop_and_reap_all_command_descendants
                # Even after the main shell succeeds. Use owned cgroup/pidfd
                # handles and kernel notifications, not sleeps or PID guesses.
                # Teardown has its own fixed internal deadline.
                remove_temporary_command_mounts_and_resolver_view
                flush_and_unmount_stage_volumes
                # Successful exit alone is not proof of filesystem durability.
                ;;
        esac
    done
}

# ---- The normal path: one supervisor, one build, in order. ----

validate_request_and_recipe
# Root execution; explicit context-relative recipe and lock paths; validate the
# complete recipe before any commands. No conventionally named YAML files.

verify_and_capture_environment_supplied_context
resolve_locked_images_from_local_readonly_cache
verify_all_referenced_content
# No downloads by conglobate. Missing Git objects, image blobs or host-architecture
# lock entries fail. Network-enabled recipe commands may download packages.

capture_external_network_inputs_if_needed
# Pin --network-namespace; capture --resolv-conf once. Both required for enabled
# steps. Environment owns interfaces, NAT, routes, firewall, DNS and teardown.

create_one_protected_tmpfs_workspace
# Proposed simplification: ordinary protected, private-propagation mount in the
# supervisor's namespace. Rust owns its descriptor, attachment and cleanup.
# Immutable image layers stay cache-backed; do not copy them into the workspace.

# No Git clone, fetch or checkout in conglobate.
retain_verified_context_bytes
# Used directly by trusted recipe/COPY processing. No second checkout or mount.

for stage in bootstrap carapace firmware; do
    if ! recipe_has_stage "$stage"; then
        continue                         # Preserve the previous output.
    fi
    select_stage_inputs_from_previous_output_or_locked_from
    case "$stage" in
        bootstrap)
            prepare_disposable_tool_root
            create_writable_snapshot_over_dm_zero
            format_output_with_trusted_formatter
            # Recipe selects filesystem/properties. Commands see tool root at /
            # and the output filesystem at /mnt. No raw block-device access.
            run_steps
            seal_output_snapshot_as_carapace
            discard_disposable_tool_root
            # Tool image supplies tools, not inherited output layers/firmware.
            ;;
        carapace)
            prepare_writable_snapshot_over_inherited_layers
            run_steps
            seal_new_layer_and_extend_carapace
            # Inherit other base-image components; bootstrap output supplies the
            # implicit base when present. Explicit from then is forbidden.
            ;;
        firmware)
            prepare_disposable_tool_root
            # Explicit from may select different tools from the output carapace.
            # Expose CARAPACE when available; references fail when unavailable.
            run_steps
            extract_and_validate_recipe_named_pmi_and_dtb
            replace_firmware_pair
            discard_disposable_tool_root
            # Firmware artifacts come from ordinary paths inside the tool root.
            # Tool filesystem changes do not become output carapace layers.
            ;;
    esac
    release_stage_resources
    # Sealed output remains owned by the session for subsequent stages/export.
done

apply_dtb_transforms_or_implicit_default
# Typed property operations, variable expansion and regex edits belong in Rust.
# An authored transform sequence replaces defaults; empty disables them.
# Regex replaces all matches; zero matches is success; wrong type is an error.
# The implicit corium edit tolerates an absent cmdline and is skipped without DTB.

assemble_self_contained_oci_layout_in_private_output_staging
write_deterministic_cbor_inventory
# Bind measured inputs to actual output identities; include all referenced blobs.
# Network configuration is not inventoried. Attestation belongs to the appliance.
verify_staged_output_and_inventory

release_captured_context_and_close_build_resources
# Explicitly check cleanup before publication. RAII handles ordinary ownership;
# destructors must not manufacture a successful quiescence/unmount receipt.

publish_output_atomically_without_replacing_existing_destination
# This is the only publication point. Failures earlier leave no successful output.
exit 0
