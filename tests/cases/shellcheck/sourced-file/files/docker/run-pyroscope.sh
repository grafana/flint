#!/bin/bash

source ./logging.sh

run_with_logging "Pyroscope ${PYROSCOPE_VERSION}" "${ENABLE_LOGS_PYROSCOPE:-false}" \
	./pyroscope/pyroscope --config.file=./pyroscope-config.yaml
