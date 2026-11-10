# Security

The stable 1.0 contract is a bounded profile for controlled generated documents.
It has not received an independent security review for unrestricted uploads. Report security issues privately using GitHub's
private vulnerability reporting for this repository; do not attach private PDFs
or exploit details to public issues.

PDF content is treated as inert data. There is no PDF JavaScript interpreter,
launch action, network access initiated by PDF operators, font substitution,
external-file resolution, encryption implementation or malformed-file repair.
Resource policies, a maximum WASM memory, worker termination deadlines and
bounded pending work reduce resource-exhaustion risk; they do not prove it absent.

The 4,000 generated parser cases are regression/property testing, not a completed
fuzzing campaign. No claim of formal verification or complete PDF conformance is
made. Native users should isolate the renderer if processing hostile input.
