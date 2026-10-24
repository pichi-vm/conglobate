# Hosted lifecycle installation timeout

Run 40046122549 at 416ca11 reached its 40-minute timeout while apt downloaded
hardware firmware from the Ubuntu Azure mirror. No conglobate test ran. Preserve
the cancelled result and full log; this is neither a lifecycle pass nor evidence
of a conglobate lifecycle failure.

The HWE meta-package depends on linux-firmware and headers. The selected image
package instead depends on its matching modules, kmod and linux-base. The workflow
now resolves the image from the HWE image meta-package metadata, requires exactly
one valid package name, and installs that image directly without recommendations.
This retains the required guest kernel/module inputs without physical hardware
firmware downloads. Raw relevant package stanzas and source hash are preserved.
The hosted retry, not this dependency inspection, must establish CI success.
