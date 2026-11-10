.PHONY: help venv deps build build-dev test stubtest clean

VENV ?= .venv
PYTHON ?= python3
UV ?= uv

PY := $(VENV)/bin/python
# maturin installs into the environment that VIRTUAL_ENV names, or else into `.venv`: without
# this, any other VENV is ignored. `--locked` keeps Cargo.lock as committed, as CI does.
MATURIN_DEVELOP := VIRTUAL_ENV="$(abspath $(VENV))" $(VENV)/bin/maturin develop --locked

help:
	@echo "Targets:"
	@echo "  venv      Create $(VENV) using uv"
	@echo "  deps      Install dev deps (maturin, pytest, mypy) into $(VENV) using uv"
	@echo "  build     Build + install extension into $(VENV) (release) via maturin"
	@echo "  build-dev Build + install extension into $(VENV) (debug) via maturin"
	@echo "  test      Run pytest (ensures build first)"
	@echo "  stubtest  Check mft.pyi against the built extension"
	@echo "  clean     Remove $(VENV) and dist/"

$(PY):
	$(UV) venv $(VENV) --python $(PYTHON)

venv: $(PY)

deps: venv
	$(UV) pip install --python $(PY) -U "maturin>=1.15,<2.0" pytest mypy typing_extensions

build: deps
	$(MATURIN_DEVELOP) --release

build-dev: deps
	$(MATURIN_DEVELOP)

test: build
	$(PY) -m pytest -q

stubtest: build-dev
	$(PY) -m mypy.stubtest mft --allowlist tests/stubtest-allowlist.txt

clean:
	rm -rf $(VENV) dist
