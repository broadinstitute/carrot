#!/usr/bin/env bash

git init > /dev/null 2>&1
touch file.txt
echo "text" > file.txt
git add file.txt > /dev/null 2>&1
git commit -am "Initial commit" > /dev/null 2>&1
GIT_COMMITTER_DATE="2022-01-01 00:00:00 +0000" git commit --amend --no-edit
first_commit_hash=$(git rev-parse HEAD)
first_commit_date=$(git show --no-patch --no-notes --pretty='%cd' ${first_commit_hash})
git tag "first" > /dev/null 2>&1
git tag "beginning" > /dev/null 2>&1
git checkout -b "test_branch"
echo "more text" >> file.txt
git commit -am "Arbitrary change" > /dev/null 2>&1
GIT_COMMITTER_DATE="2022-11-01 00:00:00 +0000" git commit --amend --no-edit
second_commit_hash=$(git rev-parse HEAD)
second_commit_date=$(git show --no-patch --no-notes --pretty='%cd' ${second_commit_hash})
echo "${first_commit_hash}\n${second_commit_hash}\n${first_commit_date}\n${second_commit_date}"
