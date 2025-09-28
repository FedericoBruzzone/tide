# Align local main to `origin/main`
git fetch -u origin main
git checkout main
git rebase origin/main

# Align local dev to `origin/dev`
git fetch -u origin dev
git checkout dev
git rebase origin/dev

# Rebase dev onto main
git checkout dev
git rebase main
