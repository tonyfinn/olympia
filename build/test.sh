#!/bin/bash

if [[ "$CI_COMMIT_BRANCH" = "master" ]]; then
    echo "Coverage with sending to coveralls"
    cargo tarpaulin --coveralls $COVERALLS_TOKEN
else
    echo "Coverage without coveralls"
    cargo tarpaulin
fi