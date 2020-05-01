#!/bin/bash

if [[ ("$CI_COMMIT_BRANCH" = "$CI_DEFAULT_BRANCH") && ("x$COVERALLS_TOKEN" != "x") ]]; then
    cargo tarpaulin --coveralls $COVERALLS_TOKEN
else
    cargo tarpaulin
fi