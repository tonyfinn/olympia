#!/bin/bash

if [[ "$CI_COMMIT_BRANCH" = "master" ]]; then
    echo "Coverage with sending to coveralls"
    git checkout master
    git reset --hard $CI_COMMIT_SHA
    cargo tarpaulin --coveralls $COVERALLS_TOKEN
else
    echo "Coverage without coveralls"
    cargo tarpaulin
fi