#!/bin/sh
# Keep the executed inode immutable while the test changes its response script.
exec /bin/sh "$0.response" "$@"
