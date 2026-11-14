<!-- SPDX-License-Identifier: CC0-1.0 -->

# Raw matrix observations · `spec:compute:matrix-observations`

## Recorded evidence · `sec:compute:matrix-evidence`

These records describe a machine under its recorded load. Sampling intervals express conditional sampling uncertainty, not numerical error or a hardware peak. They neither supply a correctness oracle nor calibrate the theoretical LC overhead reference (`rule:nrz:compute-design-authority`).

No admitted compute observations are available.

Repository summary admission remains disabled until the adopted design policy is implemented (`req:nrz:compute-admission-activation`).

Matrix rates report aggregate items and output elements; each accepted item contains the recorded number of kernel traversals. No logical-traffic rate is assigned from output bytes alone; backend packing and repeated internal accesses are not inferred from shape.

Prepared GEMM and GEMV repetitions overwrite their complete output without an extra timed zero fill. Repeated prepared solves restore the right-hand side between repetitions inside the timer. Restoration-inclusive repeated factors copy their immutable input before every factorization, including the first; their rate includes this copy.

Raw reports are retained externally. A summary-only reader cannot verify an unseen raw digest, reconstruct chronology or independently reanalyze omitted observations. The admission receipt states the actual verification scope (`rule:nrz:compute-design-admission`).

Shapes that may enter opaque backend packing are reported as `BackendPacking` unsupported outcomes under (`open:nrz:compute-backend-packing-requirement`); no measured zero rate or scalar replacement is implied.

GEMM supports only shapes with at least one dimension equal to 1. Factor and solve support order 1 only; other shapes report `Unsupported(BackendPacking)` under (`open:nrz:compute-backend-packing-requirement`).
