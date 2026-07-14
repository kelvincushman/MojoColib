.PHONY: check docs rust web upstream

check: docs rust web upstream

docs:
	python3 tools/check_docs.py
	python3 -m compileall -q tools fixtures
	python3 fixtures/tiny_training/generate.py --check
	cd fixtures/tiny_training && python3 -m unittest test_fixture.py
	cd fixtures/protocol && python3 -m unittest test_canonical.py
	git diff --check

rust:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	MOJOCOLIB_TEST_SECRET=parent-only cargo test --workspace

web:
	cd apps/control-plane-web && npm ci
	cd apps/control-plane-web && npm test
	cd apps/control-plane-web && npm run build

upstream:
	$(MAKE) -C engine/colibri check
