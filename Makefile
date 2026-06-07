# AION-MEDSAFE developer tasks.
.PHONY: help audit build test fmt release

help:
	@echo "AION-MEDSAFE make targets:"
	@echo "  make build   - build the release binary (system/)"
	@echo "  make test    - run Rust + Python test suites"
	@echo "  make fmt     - format Rust (system/)"
	@echo "  make audit   - full system integrity audit (scripts/audit.sh)"
	@echo "  make release - build + seal a SLSA-style release attestation"

build:
	cd system && cargo build --release

test:
	cd system && cargo test
	cd pipeline && PYTHONPATH=src python3 -m pytest tests/ -q

fmt:
	cd system && cargo fmt

audit:
	./scripts/audit.sh

release:
	cd system && cargo build --release && ./target/release/aion-medsafe release
