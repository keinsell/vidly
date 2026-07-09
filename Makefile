DATABASE_URL ?= data/vidly.db

.PHONY: db-install db-create db-setup db-migrate db-redo db-revert db-reset \
        db-schema db-studio db-new db-seed

# Install diesel CLI (required for migration management)
db-install:
	cargo install diesel_cli --no-default-features --features sqlite

# Generate a new migration: make db-new name=<description>
db-new:
	@if [ -z "$(name)" ]; then \
		echo "Usage: make db-new name=<description>"; \
		exit 1; \
	fi
	diesel migration generate --database-url $(DATABASE_URL) $(name)

# Create the database file (without running migrations)
db-create:
	mkdir -p $(dir $(DATABASE_URL))
	diesel database create --database-url $(DATABASE_URL)

# Create database and run all pending migrations
db-setup:
	mkdir -p $(dir $(DATABASE_URL))
	diesel database setup --database-url $(DATABASE_URL)

# Run all pending migrations
db-migrate:
	diesel migration run --database-url $(DATABASE_URL)

# Rollback and re-run the last migration
db-redo:
	diesel migration redo --database-url $(DATABASE_URL)

# Rollback the last migration
db-revert:
	diesel migration revert --database-url $(DATABASE_URL)

# Drop the database and re-run all migrations
db-reset:
	diesel database reset --database-url $(DATABASE_URL)

# Print the current database schema as Rust code
db-schema:
	diesel print-schema --database-url $(DATABASE_URL)

# Open an interactive SQLite shell for the database
db-studio:
	@if command -v sqlite3 >/dev/null 2>&1; then \
		sqlite3 $(DATABASE_URL); \
	elif command -v litecli >/dev/null 2>&1; then \
		litecli $(DATABASE_URL); \
	else \
		echo "No SQLite CLI found. Install one of:"; \
		echo "  brew install sqlite3"; \
		echo "  pip install litecli"; \
	fi
