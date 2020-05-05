#!/bin/bash

if [[ "$CI_COMMIT_BRANCH" = "master" ]]; then
    echo "Coverage with sending to coveralls"
    # Set HEAD to master for tarpaulin coveralls report
    git reset --soft master
    git checkout master
    cargo tarpaulin --coveralls $COVERALLS_TOKEN
else
    echo "Coverage without coveralls"
    cargo tarpaulin
fi