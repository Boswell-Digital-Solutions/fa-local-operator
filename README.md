# bds · FA Local Operator

> **System identity — bds family (Boswell Digital Solutions business system, local-systems tier).**
> The governed local execution operator within the Forge **ecosystem backend**; part of `ecosystem/local-systems`.
> **Purpose:** business-side local runtime — backend/ecosystem execution, not public-app support.
> **Not the Forge counterpart:** the public-app support boundary is `apps/public-app-local-support/fa-local` (Forge family).

FA Local is the governed local execution boundary for Forge applications.

This repository is the implementation home for the FA Local service. It is intentionally separate from `forge-local-runtime`, which remains the governance-and-contracts authority repository for the shared local runtime layer.

Current status: Ticket 1 scaffold complete. The crate builds, exposes typed baseline vocabulary, and defaults toward fail-closed admission. Contract schemas, artifact loaders, and execution coordination are intentionally not implemented yet.
