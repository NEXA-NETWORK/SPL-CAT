#!/bin/bash

if [ $# -eq 0 ]; then
  echo "No arguments provided."
else
  # Run the test
  echo "Running test..."
  if [ $1 == "all" ]; then
    echo "Running all tests..."
    anchor build && anchor deploy
    anchor test --skip-local-validator
  elif [ $1 == "test" ]; then
    echo "Running test..."
    anchor build -p test_token && anchor deploy -p test_token
  elif [ $1 == "build" ]; then
    echo "Running all Deployments..."
    anchor build && anchor deploy
  elif [ $1 == "new" ]; then
    echo "Running CATSOL20 test..."
    anchor build -p cat_sol20 && anchor deploy -p cat_sol20
    anchor test --run tests/CATSOL20 --skip-build --skip-deploy --skip-local-validator
  elif [ $1 == "proxy" ]; then
    echo "Running CATSOL20Proxy test..."
    anchor build -p cat_sol20_proxy && anchor deploy -p cat_sol20_proxy
    anchor test --run tests/CATSOL20Proxy --skip-build --skip-deploy --skip-local-validator
  fi

fi
