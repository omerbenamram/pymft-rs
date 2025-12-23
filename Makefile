.PHONY: help venv deps build build-dev test clean

VENV ?= .venv
PYTHON ?= python3
UV ?= uv

PY := $(VENV)/bin/python

help:
	@echo "Targets:"
	@echo "  venv      Create $(VENV) using uv"
	@echo "  deps      Install dev deps (maturin, pytest) into $(VENV) using uv"
	@echo "  build     Build + install extension into $(VENV) (release, abi3) via maturin"
	@echo "  build-dev Build + install extension into $(VENV) (debug, abi3) via maturin"
	@echo "  test      Run pytest (ensures build first)"
	@echo "  clean     Remove $(VENV) and dist/"

$(PY):
	$(UV) venv $(VENV) --python $(PYTHON)

venv: $(PY)

deps: venv
	$(UV) pip install --python $(PY) -U "maturin>=1.0,<2.0" pytest

build: deps
	$(VENV)/bin/maturin develop --release --features abi3

build-dev: deps
	$(VENV)/bin/maturin develop --features abi3

test: build
	$(PY) -m pytest -q

clean:
	rm -rf $(VENV) dist


