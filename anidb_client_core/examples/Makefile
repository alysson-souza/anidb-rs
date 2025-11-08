# Makefile for AniDB C examples

CC = clang
CFLAGS = -Wall -Wextra -O2 -std=c11
INCLUDES = -I../include

# Use release build by default
BUILD_TYPE ?= release
ifeq ($(BUILD_TYPE),debug)
    LDFLAGS = -L../target/debug -lanidb_client_core
    LIB_PATH = ../target/debug
else
    LDFLAGS = -L../target/release -lanidb_client_core
    LIB_PATH = ../target/release
endif

# Platform-specific settings
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Linux)
    LDFLAGS += -lpthread -ldl -lm
    RUN_PREFIX = LD_LIBRARY_PATH=$(LIB_PATH):$$LD_LIBRARY_PATH
endif
ifeq ($(UNAME_S),Darwin)
    LDFLAGS += -lpthread
    RUN_PREFIX = DYLD_LIBRARY_PATH=$(LIB_PATH):$$DYLD_LIBRARY_PATH
endif

# All C example targets
TARGETS = callback_demo c_basic_example c_advanced_example c_error_handling

.PHONY: all clean $(TARGETS) run-all

all: $(TARGETS)

# Build rules for each example
callback_demo: callback_demo.c
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $< $(LDFLAGS)

c_basic_example: c_basic_example.c
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $< $(LDFLAGS)

c_advanced_example: c_advanced_example.c
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $< $(LDFLAGS)

c_error_handling: c_error_handling.c
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $< $(LDFLAGS)

# Clean all built examples
clean:
	rm -f $(TARGETS)
	rm -f *.log

# Run examples
run-callback: callback_demo
	$(RUN_PREFIX) ./callback_demo ../tests/fixtures/small_file.bin || echo "Note: Create test file first"

run-basic: c_basic_example
	@echo "Running basic example..."
	@echo "Usage: ./c_basic_example <file_path>"
	$(RUN_PREFIX) ./c_basic_example ../tests/fixtures/small_file.bin || echo "Note: Create test file first"

run-advanced: c_advanced_example
	@echo "Running advanced example..."
	@echo "Usage: ./c_advanced_example <file1> [file2] [file3] ..."
	$(RUN_PREFIX) ./c_advanced_example ../tests/fixtures/small_file.bin || echo "Note: Create test file first"

run-error: c_error_handling
	@echo "Running error handling example..."
	$(RUN_PREFIX) ./c_error_handling || true

run-all: all
	@echo "Running all examples..."
	@$(MAKE) run-callback
	@echo "\n---\n"
	@$(MAKE) run-basic
	@echo "\n---\n"
	@$(MAKE) run-advanced
	@echo "\n---\n"
	@$(MAKE) run-error

# Build with debug symbols
debug: CFLAGS += -g -DDEBUG
debug: BUILD_TYPE = debug
debug: clean all

# Help
help:
	@echo "AniDB C Examples Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  all              - Build all examples"
	@echo "  clean            - Remove built examples and logs"
	@echo "  debug            - Build with debug symbols"
	@echo "  run-all          - Run all examples"
	@echo "  run-callback     - Run callback demo"
	@echo "  run-basic        - Run basic example"
	@echo "  run-advanced     - Run advanced example"
	@echo "  run-error        - Run error handling example"
	@echo ""
	@echo "Options:"
	@echo "  BUILD_TYPE=debug - Use debug build (default: release)"
	@echo ""
	@echo "Example:"
	@echo "  make BUILD_TYPE=debug run-basic"