#!/bin/bash
# Wrapper entrypoint for Pelican/Pterodactyl compatibility
# Passes through whatever command the panel sends
exec "$@"
