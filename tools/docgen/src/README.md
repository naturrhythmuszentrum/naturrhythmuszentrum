<!-- SPDX-License-Identifier: CC0-1.0 -->

# Docgen src evidence · `guide:docgen:src-evidence`

**Rule (Declared input layout)** · `rule:docgen:input-layout`

The root Cargo manifest declares workspace members, the document manifest under `data/` declares rendered outputs, and the project input under `data/inputs/` supplies authored project metadata; generation reads those canonical sources and refuses alternate layouts rather than searching for substitutes.

**Rule (Distinct artifact identities)** · `rule:docgen:dataset-identities`

Each input family, output manifest and generated dataset has a distinct namespace under the repository's declared namespace hierarchy; dataset filenames under `data/datasets/` match the names exposed to templates, while problem and fixture namespaces preserve their distinct source-tree roles.

**Rule (Reserved envelope order)** · `rule:docgen:envelope-order`

The envelope orders namespace, version, owner, license, sources and labels before payload fields; decoding reserves those names, preserves array order and sorts object payload keys so generation has a stable byte representation.

**Rule (Declared owner authority)** · `rule:docgen:owner-authority`

Envelope owners come from the linter owner declaration under `.linter/`; a separate generator-owned owner list would permit the documentation and label gates to disagree about admissible ownership.

## Unit test matrix · `tab:docgen:unit-test-matrix`

**Table (Unit test matrix)**

|Test|Area|Claim|
|----|----|-----|
|(`test:unit:census-across-workspace-roots`)|docgen|Use explicit members across all three roots and only mirrored README sources.|
|(`test:unit:derives-member-order-and-optional-facts`)|docgen|Keep member order, inherit one description, and represent absent values distinctly.|
|(`test:unit:refuses-invalid-member-paths`)|docgen|Reject globs, parent traversal, and duplicates before reading member manifests.|
|(`test:unit:census-dependency-identities-and-refusals`)|docgen|Manifest edges retain aliases, workspace inheritance and dependency kinds while refusing conditional declarations.|
|(`test:unit:accepts-supported-major-and-newer-minor`)|docgen|Accept major one with newer minor and patch versions.|
|(`test:unit:checks-problem-schema-version-policy`)|docgen|Admit only the named v2 minor and reject legacy data with a migration diagnostic.|
|(`test:unit:rejects-invalid-envelopes`)|docgen|Reject missing fields, namespace and version errors, owners, and labels.|
|(`test:unit:envelope-error-precedes-payload-error`)|docgen|Reserve envelope validation ahead of an invalid typed payload.|
|(`test:unit:serializer-orders-fields-and-preserves-arrays`)|docgen|Serialize envelope keys first and sort nested objects without reordering arrays.|
|(`test:unit:refuses-invalid-document-rows`)|docgen|Refuse source-directory outputs, escaping paths, and repeated outputs.|
|(`test:unit:refuses-explicit-template-path`)|docgen|Refuse even a correctly mirrored template override in the manifest.|
|(`test:unit:decodes-valid-problem-and-fixture`)|docgen|Decode one valid problem and one valid fixture into the ordered dataset.|
|(`test:unit:layout-refuses-stray-file`)|docgen|Reject a stray file at the top of the problems tree.|
|(`test:unit:layout-refuses-second-file`)|docgen|Reject a second file inside an otherwise valid slug directory.|
|(`test:unit:layout-refuses-malformed-slug`)|docgen|Reject a slug with a repeated hyphen.|
|(`test:unit:envelope-refuses-wrong-namespace`)|docgen|Reject an input namespace that disagrees with its tree location.|
|(`test:unit:envelope-refuses-second-label`)|docgen|Reject a second envelope label even when both labels are well formed.|
|(`test:unit:identity-refuses-register-mismatch`)|docgen|Catch a register that differs from the envelope label.|
|(`test:unit:values-refuse-invalid-number`)|docgen|Reject a value string outside the exact grammar.|
|(`test:unit:units-refuse-unknown-key`)|docgen|Reject an undeclared descriptor field.|
|(`test:unit:uncertainty-refuses-incomplete-shape`)|docgen|Reject an enclosure that omits its source. TODO ´todo:code:reject-missing-correlations-under-the-future´: reject missing correlations under the future shared-source contract without treating standard uncertainty as a radius under rule:vision:uncertainty-unit-algebra.|
|(`test:unit:exactness-refuses-wrong-precision`)|docgen|Reject a decimal with fewer significant digits than declared.|
|(`test:unit:labels-refuse-wrong-bite-kind`)|docgen|Reject a bite label outside the step and NRZ area.|
|(`test:unit:envelope-precedes-payload-error`)|docgen|Report an invalid envelope before attempting to decode an invalid payload.|
|(`test:unit:refuses-requirement-mismatch`)|docgen|Reject a declared dataset that differs from the manifest row.|
|(`test:unit:refuses-wrong-dataset-major`)|docgen|Reject a template major that differs from the dataset's major.|
