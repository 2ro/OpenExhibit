# Generate and audit every doc under docs/.
#
# `make docs` re-runs `scripts/gen-docs.sh`, which:
#   - cross-checks the Markdown files against source (routes, formats,
#     migrations, env vars) and fails if anything drifts.
#   - rebuilds the cargo rustdoc HTML at target/doc/openexhibit/.
#
# Run from the project root: `make -C docs` or `make -C docs docs`.

PROJECT_ROOT := $(abspath $(CURDIR)/..)
GEN_SCRIPT   := $(PROJECT_ROOT)/scripts/gen-docs.sh

# Track the source files the markdown docs claim to derive from so
# `make docs` triggers when anything they depend on actually moves.
MD_SOURCES := \
    $(wildcard $(PROJECT_ROOT)/src/*.rs) \
    $(wildcard $(PROJECT_ROOT)/src/routes/*.rs) \
    $(wildcard $(PROJECT_ROOT)/src/routes/admin/*.rs) \
    $(wildcard $(PROJECT_ROOT)/src/formats/*.rs) \
    $(wildcard $(PROJECT_ROOT)/src/models/*.rs) \
    $(wildcard $(PROJECT_ROOT)/migrations/*.sql) \
    $(PROJECT_ROOT)/.env.example

MD_FILES := \
    $(CURDIR)/architecture.md \
    $(CURDIR)/api-routes.md \
    $(CURDIR)/database-schema.md \
    $(CURDIR)/exhibit-formats.md \
    $(CURDIR)/configuration.md \
    $(CURDIR)/README.md

RUSTDOC_INDEX := $(PROJECT_ROOT)/target/doc/openexhibit/index.html

.PHONY: docs check clean help

docs: $(RUSTDOC_INDEX)
	@$(GEN_SCRIPT)

# Audit-only — no cargo doc rebuild. Faster in CI when API docs are
# regenerated elsewhere.
check:
	@cd $(PROJECT_ROOT) && \
	  PATH="$$PATH" \
	  bash -c 'set -e; for fn in audit_routes audit_formats audit_migrations audit_env; do \
	    source $(GEN_SCRIPT); $$fn; done'

$(RUSTDOC_INDEX): $(MD_SOURCES)
	cd $(PROJECT_ROOT) && cargo doc --no-deps --quiet

clean:
	rm -rf $(PROJECT_ROOT)/target/doc

help:
	@echo "Targets:"
	@echo "  docs   - audit Markdown against source AND rebuild rustdoc HTML"
	@echo "  check  - audit Markdown only (no rustdoc rebuild)"
	@echo "  clean  - remove target/doc/"
