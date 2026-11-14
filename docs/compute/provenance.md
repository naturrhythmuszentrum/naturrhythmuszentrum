<!-- SPDX-License-Identifier: CC0-1.0 -->

# Compute provenance · `spec:compute:provenance-observations`

## Recorded evidence · `sec:compute:provenance-evidence`

These records describe a machine under its recorded load. Sampling intervals express conditional sampling uncertainty, not numerical error or a hardware peak. They neither supply a correctness oracle nor calibrate the theoretical LC overhead reference (`rule:nrz:compute-design-authority`).

No admitted compute observations are available.

Repository summary admission remains disabled until the adopted design policy is implemented (`req:nrz:compute-admission-activation`).

Raw reports are retained externally. A summary-only reader cannot verify an unseen raw digest, reconstruct chronology or independently reanalyze omitted observations. The admission receipt states the actual verification scope (`rule:nrz:compute-design-admission`).

## Changed build and environment evidence · `sec:compute:provenance-changes`

Adjacent admitted records retain separate results. These tables identify metadata changes; equal unavailable values do not establish equal machines or builds. A changed field is not evidence of a performance cause. Eligibility is the review receipt's decision, and unlike case scopes remain separate (`rule:nrz:compute-comparison`).

Known source attachments use reviewed repository-relative patch links and full blob identities. Admission checks the bytes and symlink policy, binds them through the summary receipt, and counts the attachment bytes against the aggregate cap. No raw observation file is an attachment (`rule:nrz:compute-design-admission`).
