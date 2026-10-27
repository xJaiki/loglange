# ==============================================================================
# Loglance - Ultra-Fast Minimalist Real-Time Log Viewer
# Makefile for local development, testing, and production builds
# ==============================================================================

PNPM  ?= pnpm
CARGO ?= cargo
TAURI ?= $(PNPM) tauri

.DEFAULT_GOAL := help
.PHONY: help install setup dev dev-web build build-debug build-frontend build-rust check test clippy fmt clean sample-log stream-log live-journal

help: ## Show this help message with available commands
	@echo "Usage: make [target]"
	@echo ""
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

install: ## Install frontend and project dependencies via pnpm
	$(PNPM) install

setup: install ## Alias for install

dev: ## Run full Tauri desktop application in development mode
	WEBKIT_DISABLE_DMABUF_RENDERER=1 $(TAURI) dev

dev-web: ## Run frontend in browser-only Vite dev mode
	$(PNPM) dev

build: ## Build release desktop application bundle (frontend + Tauri)
	$(TAURI) build

build-debug: ## Build debug desktop application without release optimizations
	$(TAURI) build --debug

build-frontend: ## Build frontend assets with SvelteKit / Vite
	$(PNPM) build

build-rust: ## Build only the Rust backend in release mode
	$(CARGO) build --release --manifest-path src-tauri/Cargo.toml

check: ## Run typechecks and cargo checks (Svelte check + Cargo check)
	$(PNPM) check
	$(CARGO) check --manifest-path src-tauri/Cargo.toml

test: ## Run Rust automated test suite
	$(CARGO) test --manifest-path src-tauri/Cargo.toml

clippy: ## Run Rust linter (cargo clippy)
	$(CARGO) clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

fmt: ## Format Rust code (cargo fmt)
	$(CARGO) fmt --manifest-path src-tauri/Cargo.toml

clean: ## Clean build artifacts (target, dist, build, .svelte-kit)
	rm -rf build dist .svelte-kit
	$(CARGO) clean --manifest-path src-tauri/Cargo.toml

sample-log: ## Generate a rich 10,000-line sample.log with JSON, errors and stacktraces
	@python3 -c '\
	import json, random, datetime;\
	levels = ["INFO", "DEBUG", "WARN", "ERROR"];\
	services = ["auth-service", "payment-gateway", "api-router", "db-pool", "worker-queue"];\
	endpoints = ["/api/v1/users", "/api/v1/checkout", "/healthz", "/metrics", "/api/v1/orders"];\
	stack_java = "   at com.example.service.OrderProcessor.processOrder(OrderProcessor.java:142)\n   at com.example.service.PaymentClient.charge(PaymentClient.java:88)\nCaused by: java.net.SocketTimeoutException: Read timed out after 5000ms\n   at java.base/sun.nio.ch.NioSocketImpl.timedRead(NioSocketImpl.java:283)";\
	now = datetime.datetime.now();\
	with open("sample.log", "w") as f:\
	    for i in range(1, 10001):\
	        ts = (now + datetime.timedelta(milliseconds=i*15)).strftime("%Y-%m-%d %H:%M:%S.%f")[:-3];\
	        lvl = random.choice(levels);\
	        srv = random.choice(services);\
	        if i % 250 == 0:\
	            f.write(f"[{ts}] ERROR [{srv}] Fatal timeout during transaction\n" + stack_java + "\n");\
	        elif i % 40 == 0:\
	            payload = json.dumps({"req_id": f"req_{i:05d}", "user": random.randint(100, 999), "status": "ok", "ms": random.randint(5, 120)});\
	            f.write(f"[{ts}] INFO [{srv}] Payload: {payload}\n");\
	        elif lvl == "WARN":\
	            f.write(f"[{ts}] WARN [{srv}] Memory limit warning at 85%\n");\
	        elif lvl == "DEBUG":\
	            f.write(f"[{ts}] DEBUG [{srv}] SQL query executed in 2ms\n");\
	        else:\
	            f.write(f"[{ts}] INFO [{srv}] Handled {random.choice(endpoints)} 200 OK\n");\
	print("Generated sample.log (10,000 lines)")'

stream-log: ## Continuously append live log lines to sample.log to test live tailing (Ctrl+C to stop)
	@echo "Streaming new log entries to sample.log (press Ctrl+C to stop)..."
	@python3 -c '\
	import time, datetime, random, sys;\
	services = ["api-router", "worker-queue", "auth-service"];\
	while True:\
	    ts = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3];\
	    line = f"[{ts}] INFO [{random.choice(services)}] Real-time event ping #{random.randint(1000, 9999)}\n";\
	    with open("sample.log", "a") as f: f.write(line);\
	    sys.stdout.write(f"Appended: {line}"); sys.stdout.flush();\
	    time.sleep(0.15)'

live-journal: ## Stream live system logs directly into Loglance via stdin pipe
	journalctl -f -o short-iso | ./src-tauri/target/debug/loglance
