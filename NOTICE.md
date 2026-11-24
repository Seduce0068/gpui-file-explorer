<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- SPDX-FileCopyrightText: 2026 Developer <developer@example.com> -->

# NOTICE — GPUI File Explorer

This repository is licensed under the **GNU Affero General Public License,
version 3 or later** (`AGPL-3.0-or-later`). See [LICENSE](LICENSE).

- Application source (`app/`): AGPL-3.0-or-later. Third-party components and
  their licenses are listed in [app/NOTICE.md](app/NOTICE.md).
- Research notes, design document and PoC (`RESEARCH.md`, `DESIGN.md`, `notes/`,
  `poc/`): AGPL-3.0-or-later unless a file states otherwise.
- Upstream dependencies keep their own licenses. The full identifier set that
  `cargo deny check` allows (this project plus its dependencies) is declared in
  the repository-root `deny.toml`: 0BSD, AGPL-3.0-or-later, Apache-2.0
  (including `Apache-2.0 WITH LLVM-exception`), BSD-2-Clause, BSD-3-Clause,
  BSL-1.0, CC0-1.0, ISC, MIT, MIT-0, MPL-2.0, Unicode-3.0, Unlicense and Zlib.
  The main components are `gpui-kit` / `gpui-pre` / `gpui-component` /
  `gpui-base` (Apache-2.0), the Lucide icon set (ISC) and `notify` (CC0-1.0);
  the remaining crates use the other identifiers listed above.

When distributing a binary built from this repository, AGPLv3 requires you to
make the corresponding source available to recipients (and to users
interacting with a modified version over a network). This repository itself is
that source: <https://github.com/Seduce0068/gpui-file-explorer>.
