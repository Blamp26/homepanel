frontend-check:
	cd frontend && npm run check

frontend-lint:
	cd frontend && npm run lint

frontend-build:
	cd frontend && npm run build

frontend-test:
	cd frontend && npm run test:e2e

frontend-validate:
	cd frontend && npm run validate

rust-check:
	cargo check

check:
	cargo check
	cd frontend && npm run validate
