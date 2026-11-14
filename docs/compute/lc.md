<!-- SPDX-License-Identifier: CC0-1.0 -->

# Naive LC observations · `spec:compute:lc-observations`

## Recorded evidence · `sec:compute:lc-evidence`

These records describe a machine under its recorded load. Sampling intervals express conditional sampling uncertainty, not numerical error or a hardware peak. They neither supply a correctness oracle nor calibrate the theoretical LC overhead reference (`rule:nrz:compute-design-authority`).

No admitted compute observations are available.

Repository summary admission remains disabled until the adopted design policy is implemented (`req:nrz:compute-admission-activation`).

Logical traversal bytes count named source operand uses and output writes: 32 bytes per forward output, 24 per element query, and 40 per inverse branch including the integer cycle input. Preparation-inclusive counts additionally include root/input initialization and reads, prepared reciprocal writes, and zero-initialized output allocation. These are recipe counts; compiler hoisting and caches can change physical traffic.

Raw reports are retained externally. A summary-only reader cannot verify an unseen raw digest, reconstruct chronology or independently reanalyze omitted observations. The admission receipt states the actual verification scope (`rule:nrz:compute-design-admission`).
