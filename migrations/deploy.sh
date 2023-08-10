#!/bin/bash

set -ex

# Delete the target/deploy/ folder
rm -rf target/deploy/
rm -rf target/idl/

# Build the project
anchor build

# Sync the keys
anchor keys sync

# Build again so it can pick up the keys
anchor build

# Deploy the project
anchor deploy --program-name cat_sol20
