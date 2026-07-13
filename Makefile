.PHONY: check docs rust upstream

check: docs rust upstream

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
	cargo test --workspace

upstream:
	$(MAKE) -C engine/colibri check
