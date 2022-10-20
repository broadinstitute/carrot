git init > /dev/null 2>&1
touch file.txt
echo "text" > file.txt
git add file.txt > /dev/null 2>&1
git commit -am "Initial commit" > /dev/null 2>&1
first_commit_hash=$(git rev-parse HEAD)
git tag "first" > /dev/null 2>&1
git tag "beginning" > /dev/null 2>&1
echo "more text" >> file.txt
git commit -am "Arbitrary change" > /dev/null 2>&1
second_commit_hash=$(git rev-parse HEAD)
echo "${first_commit_hash} ${second_commit_hash}"

